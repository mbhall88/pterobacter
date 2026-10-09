import pathlib
import tempfile
import unittest
from metrics import delta_metrics, paf_metrics, union_bases


class MetricContractTests(unittest.TestCase):
    def test_inclusive_union_keeps_singletons_and_reverse_overlap(self):
        self.assertEqual(union_bases([(1, 1), (5, 2), (4, 8)]), 8)

    def test_delta_span_error_and_four_role_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'x.filter'
            path.write_text('r.fa q.fa\nNUCMER\n>r q 10 12\n1 5 2 6 1 0 0\n0\n4 8 10 6 2 0 0\n0\n')
            result = delta_metrics(path, {'r': 10}, {'q': 12})
            self.assertEqual((result['D'], result['E'], result['reference_covered'], result['query_covered']), (20, 3, 8, 9))
            self.assertEqual(result['identity'], 70.0)
            self.assertEqual(result['query_af'], 75.0)

    def test_valid_empty_delta_is_no_alignment(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'x.filter'
            path.write_text('r.fa q.fa\nNUCMER\n')
            result = delta_metrics(path, {'r': 10}, {'q': 12})
            self.assertEqual(result['status'], 'no_alignment')
            self.assertIsNone(result['identity'])
            self.assertEqual(result['reference_af'], 0)

    def test_truncated_delta_is_error(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'x.filter'
            path.write_text('r.fa q.fa\nNUCMER\n>r q 10 12\n1 5 2 6 1 0 0\n')
            with self.assertRaises(ValueError):
                delta_metrics(path, {'r': 10}, {'q': 12})

    def test_paf_indel_identity_uses_both_spans_and_union(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'x.paf'
            path.write_text('q\t12\t0\t6\t+\tr\t10\t0\t5\t5\t6\t60\tNM:i:1\tcg:Z:3M1I2M\n')
            result = paf_metrics(path, {'r': 10}, {'q': 12})
            self.assertEqual((result['D'], result['E']), (11, 1))
            self.assertAlmostEqual(result['identity'], 100 * 9 / 11)

    def test_binding_soft_clips_do_not_inflate_alignment_spans(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'x.paf'
            path.write_text('q\t12\t2\t7\t+\tr\t10\t0\t5\t5\t5\t60\tNM:i:0\tcg:Z:2S5M5S\n')
            result = paf_metrics(path, {'r': 10}, {'q': 12})
            self.assertEqual((result['D'], result['query_covered']), (10, 5))

    def test_overlapping_paf_blocks_do_not_exceed_whole_length(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'x.paf'
            path.write_text('q\t10\t0\t6\t+\tr\t10\t0\t6\t6\t6\t60\tNM:i:0\tcg:Z:6M\n'
                            'q\t10\t3\t9\t+\tr\t10\t3\t9\t6\t6\t60\tNM:i:0\tcg:Z:6M\n')
            result = paf_metrics(path, {'r': 10}, {'q': 10})
            self.assertEqual((result['D'], result['reference_af'], result['query_af']), (24, 90.0, 90.0))

    def test_boundary_gates_are_exact(self):
        from metrics import metric_record
        exact = metric_record(200, 4, {'r': [(1, 80)]}, {'q': [(1, 80)]}, {'r': 100}, {'q': 100}, 1)
        self.assertFalse(exact['identity_gt96'])
        self.assertTrue(exact['reference_af_ge80'])
        below = metric_record(200, 3, {'r': [(1, 79)]}, {'q': [(1, 80)]}, {'r': 100}, {'q': 100}, 1)
        self.assertTrue(below['identity_gt96'])
        self.assertFalse(below['reference_af_ge80'])

    def test_out_of_bounds_and_invalid_nm_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'x.paf'
            path.write_text('q\t12\t0\t13\t+\tr\t10\t0\t5\t5\t6\t60\tNM:i:1\tcg:Z:3M1I2M\n')
            with self.assertRaises(ValueError):
                paf_metrics(path, {'r': 10}, {'q': 12})

if __name__ == '__main__':
    unittest.main()
