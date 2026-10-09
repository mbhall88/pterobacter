import pathlib
import tempfile
import unittest
from compare_mummer4 import canonical_delta


class CompleteBlockOrderTests(unittest.TestCase):
    def test_context_multiplicity_and_internal_offset_order_are_preserved(self):
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d)/'x.delta'
            a='>r q 40 40\n1 20 1 20 2 2 0\n5\n-7\n0\n'
            b='>r2 q 40 40\n1 10 1 10 0 0 0\n0\n'
            p.write_text('r q\nNUCMER\n'+a+b)
            expected=canonical_delta(p)
            p.write_text('different paths\nNUCMER\n'+b+a)
            self.assertEqual(canonical_delta(p),expected)
            p.write_text('r q\nNUCMER\n'+a.replace('5\n-7','-7\n5')+b)
            self.assertNotEqual(canonical_delta(p),expected)
            p.write_text('r q\nNUCMER\n'+a+a+b)
            self.assertNotEqual(canonical_delta(p),expected)
            p.write_text('r q\nNUCMER\n'+a.replace('>r q','>other q')+b)
            self.assertNotEqual(canonical_delta(p),expected)

    def test_truncated_block_is_not_canonicalized(self):
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d)/'x.delta'
            p.write_text('r q\nNUCMER\n>r q 40 40\n1 20 1 20 2 2 0\n5\n')
            with self.assertRaisesRegex(ValueError,'truncated'):canonical_delta(p)
