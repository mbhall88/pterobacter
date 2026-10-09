// Development bridge only. Upstream code keeps its own component licenses.
#include <mummer/nucmer.hpp>
#include <mummer/delta.hh>
#include <fstream>
#include <mutex>
#include <sstream>
#include <stdexcept>
#include <cstdlib>
#include <cstring>

// v4.0.1 reference-N replacement and delta LIS tie-breaking use libc rand().
// Serialize/reseed our calls, reproducing separate CLI processes. Other users
// of libc rand() in a host application would need separate isolation/review.
static std::mutex rng_mutex;

struct CoutRedirect {
  std::streambuf* prior;
  explicit CoutRedirect(std::ostream& output) : prior(std::cout.rdbuf(output.rdbuf())) {}
  ~CoutRedirect() { std::cout.rdbuf(prior); }
};

extern "C" int mummer4_pair_orientation(const char* reference, const char* query,
                                       const char* prefix, unsigned threads,
                                       char* error, size_t capacity) {
  try {
    const std::string delta = std::string(prefix) + ".delta";
    const std::string filtered = std::string(prefix) + ".filter";
    std::unique_ptr<mummer::nucmer::FileAligner> aligner;
    {
      std::lock_guard<std::mutex> lock(rng_mutex);
      std::srand(1);
      mummer::nucmer::Options options;
      options.mum();
      aligner = std::make_unique<mummer::nucmer::FileAligner>(reference, options);
    }
    std::ofstream output(delta);
    if (!output) throw std::runtime_error("cannot open delta output");
    output << reference << ' ' << query << "\nNUCMER\n";
    std::mutex output_mutex;
    auto print = [&](std::vector<mummer::postnuc::Alignment>&& als,
                                  const mummer::nucmer::FastaRecordPtr& ref,
                                  const mummer::nucmer::FastaRecordSeq& qry) {
      std::ostringstream buffer;
      mummer::postnuc::printDeltaAlignments(als, ref.Id(), ref.len(), qry.Id(), qry.len(), buffer, 0);
      std::lock_guard<std::mutex> lock(output_mutex);
      output << buffer.str();
    };
    // v4.0.1 align_file's trampoline template fails to instantiate on GCC11.
    // Use its public thread_align_file path, as the pinned CLI itself does.
    using Streams = jellyfish::stream_manager<const char**>;
    using Parser = jellyfish::whole_sequence_parser<Streams>;
    Streams streams(&query, &query + 1);
    Parser parser(4 * threads, 10, 1, 1, streams);
    std::vector<std::thread> workers;
    for (unsigned i = 0; i < threads; ++i)
      workers.emplace_back([&] { aligner->thread_align_file(parser, print); });
    for (auto& worker : workers) worker.join();
    output.close();
    if (!output) throw std::runtime_error("cannot write delta output");
    std::ofstream filtered_output(filtered);
    if (!filtered_output) throw std::runtime_error("cannot open filtered output");
    {
      std::lock_guard<std::mutex> lock(rng_mutex);
      std::srand(1);
      DeltaGraph_t graph;
      graph.build(delta, true);
      graph.flag1to1(-1.0, 100.0);
      // v4.0.1 outputDelta ignores its ostream argument and writes cout.
      // Confine redirection to the serialized filter section; restore on error.
      CoutRedirect redirect(filtered_output);
      graph.outputDelta(filtered_output);
    }
    filtered_output.close();
    if (!filtered_output) throw std::runtime_error("cannot write filtered output");
    return 0;
  } catch (const std::exception& exception) {
    if (capacity) {
      std::strncpy(error, exception.what(), capacity - 1);
      error[capacity - 1] = '\0';
    }
    return 1;
  }
}
