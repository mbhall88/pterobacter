// Pterobacter addition, MIT licensed. No glibc source is included.
#ifndef PTEROBACTER_FILTER_RNG_HH
#define PTEROBACTER_FILTER_RNG_HH
#include <array>
#include <cstddef>
#include <cstdint>

namespace pterobacter {
// Seed-1 glibc random()/rand() sequence: degree-31 additive generator,
// separation 3, Park-Miller initialization and 310 discarded outputs.
// Explicit unsigned arithmetic defines modulo-2^32 addition on every target.
// This is private to one delta graph; it never reads or changes libc RNG state.
class FilterRng {
  std::array<std::uint32_t, 31> state_{};
  std::size_t front_ = 3;
  std::size_t rear_ = 0;
public:
  FilterRng() {
    state_[0] = 1;
    for (std::size_t i = 1; i < state_.size(); ++i)
      state_[i] = (std::uint64_t(state_[i - 1]) * 16807) % 2147483647;
    for (int i = 0; i < 310; ++i) next();
  }
  std::uint32_t next() {
    state_[front_] += state_[rear_];
    const auto value = state_[front_] >> 1;
    front_ = (front_ + 1) % state_.size();
    rear_ = (rear_ + 1) % state_.size();
    return value;
  }
  int choose(long count) {
    // Preserve the upstream double arithmetic and truncation, including
    // consumption of one draw when count is 1.
    return int(double(count) * next() / 2147483648.0);
  }
};
}
#endif
