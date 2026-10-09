// Upstream components retain their own licenses and notices.
#include <mummer/nucmer.hpp>
#include <mummer/delta.hh>
#include <fstream>
#include <sstream>
#include <stdexcept>
#include <cstdlib>
#include <cstring>

// Only called inside an isolated worker, once per orientation. Fatal exits and
// exceptions escaping upstream threads cannot kill the coordinating process.
// libc RNG is private to the worker; retain CLI-compatible seed-1 tie handling.
extern "C" int tero_anim_orientation(const char* reference, const char* query,
                                     const char* prefix, char* error, size_t capacity) {
  try {
    mummer::nucmer::Options options;
    options.mum();
    mummer::nucmer::FileAligner aligner(reference, options);
    const std::string delta = std::string(prefix) + ".delta";
    std::ofstream output(delta);
    if (!output) throw std::runtime_error("cannot open delta output");
    output << reference << ' ' << query << "\nNUCMER\n";
    auto print = [&](std::vector<mummer::postnuc::Alignment>&& alignments,
                     const mummer::nucmer::FastaRecordPtr& ref,
                     const mummer::nucmer::FastaRecordSeq& qry) {
      mummer::postnuc::printDeltaAlignments(alignments, ref.Id(), ref.len(),
                                         qry.Id(), qry.len(), output, 0);
    };
    aligner.align_file(query, print, 1);
    output.close();
    if (!output) throw std::runtime_error("cannot write delta output");
    std::ofstream filtered(std::string(prefix) + ".filter");
    if (!filtered) throw std::runtime_error("cannot open filtered output");
    std::srand(1);
    DeltaGraph_t graph;
    graph.build(delta, true);
    graph.flag1to1(-1.0, 100.0);
    graph.outputDelta(filtered);
    filtered.close();
    if (!filtered) throw std::runtime_error("cannot write filtered output");
    return 0;
  } catch (const std::exception& e) {
    if (capacity) { std::strncpy(error, e.what(), capacity - 1); error[capacity - 1] = '\0'; }
    return 1;
  } catch (...) {
    if (capacity) { std::strncpy(error, "unknown native exception", capacity - 1); error[capacity - 1] = '\0'; }
    return 1;
  }
}
