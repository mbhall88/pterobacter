// Synthetic native regression; no study sequences or evidence.
#include <mummer/delta.hh>
#include <mummer/pterobacter_filter_rng.hh>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>

static void require(bool ok) { if (!ok) throw std::runtime_error("filter regression failed"); }
int main(int argc, char** argv) {
  require(argc == 3);
  const std::uint32_t first[] = {1804289383, 846930886, 1681692777, 1714636915,
    1957747793, 424238335, 719885386, 1649760492, 596516649, 1189641421};
  pterobacter::FilterRng stream;
  for (auto expected : first) require(stream.next() == expected);
  // Long fixed-vector fingerprint is portable, including non-glibc platforms.
  pterobacter::FilterRng long_stream;
  std::uint64_t fingerprint = 14695981039346656037ULL;
  for (int i = 0; i < 100000; ++i) {
    const auto value = long_stream.next();
    for (int byte = 0; byte < 4; ++byte) {
      fingerprint ^= (value >> (byte * 8)) & 255;
      fingerprint *= 1099511628211ULL;
    }
  }
  require(fingerprint == 0x930fde5f6b0cf683ULL);
#ifdef __GLIBC__
  // Independent platform reference is only valid on glibc, never on macOS.
  std::srand(1);
  pterobacter::FilterRng reference;
  for (int i = 0; i < 100000; ++i) require(reference.next() == std::uint32_t(std::rand()));
#endif
  std::ifstream expected_file(argv[2]);
  std::ostringstream expected;
  expected << expected_file.rdbuf();
  require(!expected.str().empty());
  for (unsigned seed : {1, 57, 999}) {
    // libc state must neither affect the result nor be consumed by filtering.
    std::srand(seed);
    const auto next_libc = std::rand();
    std::srand(seed);
    DeltaGraph_t graph;
    graph.build(argv[1], true);
    graph.flag1to1(-1.0, 100.0);
    std::ostringstream result;
    graph.outputDelta(result);
    require(result.str() == expected.str());
    require(std::rand() == next_libc);
    // Reuse the same graph object to prove each filtering call resets its stream.
    graph.clear();
    graph.build(argv[1], true);
    graph.flag1to1(-1.0, 100.0);
    std::ostringstream repeated;
    graph.outputDelta(repeated);
    require(repeated.str() == expected.str());
  }
  std::cout << "fixed vectors, 100000 draws, real tied filter, reset, libc isolation: PASS\n";
}
