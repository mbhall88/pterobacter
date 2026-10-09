// Development-only shared-process diagnostic. Upstream licenses still apply.
#include <atomic>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

extern "C" int mummer4_pair_orientation(const char*, const char*, const char*,
                                       unsigned, char*, size_t);
struct Job {
  std::string reference, query, prefix;
};

int main(int argc, char** argv) {
  if (argc != 3) return 2;
  const unsigned workers = std::stoul(argv[2]);
  if (workers != 1 && workers != 8) return 2;
  std::ifstream input(argv[1]);
  if (!input) return 2;
  std::vector<Job> jobs;
  std::string line;
  while (std::getline(input, line)) {
    std::istringstream fields(line);
    Job job;
    if (!std::getline(fields, job.reference, '\t') ||
        !std::getline(fields, job.query, '\t') ||
        !std::getline(fields, job.prefix) || job.prefix.empty()) return 2;
    jobs.push_back(job);
  }
  std::atomic<size_t> next{0};
  std::vector<int> codes(jobs.size(), -1);
  std::vector<std::string> errors(jobs.size());
  std::vector<std::thread> threads;
  for (unsigned i = 0; i < workers; ++i) {
    threads.emplace_back([&] {
      for (;;) {
        const size_t index = next.fetch_add(1);
        if (index >= jobs.size()) break;
        const auto& job = jobs[index];
        char error[1024] = {};
        codes[index] = mummer4_pair_orientation(job.reference.c_str(),
            job.query.c_str(), job.prefix.c_str(), 1, error, sizeof(error));
        errors[index] = error;
      }
    });
  }
  for (auto& thread : threads) thread.join();
  bool failed = false;
  for (size_t i = 0; i < jobs.size(); ++i) {
    std::cout << i << '\t' << codes[i] << '\t' << errors[i] << '\n';
    failed |= codes[i] != 0;
  }
  return failed ? 1 : 0;
}
