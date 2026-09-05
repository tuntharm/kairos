#pragma once

#include <cstdint>

namespace kairos {

constexpr std::uint32_t kSessionsPerRank = 4;

struct CharacterProgress {
  std::uint32_t rank{1};
  std::uint32_t xp_in_rank{0};
  std::uint32_t sessions{0};
};

inline CharacterProgress character_progress(std::uint32_t completed_sessions) {
  return CharacterProgress{
      1U + completed_sessions / kSessionsPerRank,
      completed_sessions % kSessionsPerRank,
      completed_sessions,
  };
}

}  // namespace kairos
