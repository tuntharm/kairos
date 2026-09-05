#pragma once

#include <cstdint>

namespace kairos {

struct PomodoroPreset {
  std::uint16_t focus_minutes{25};
  std::uint16_t break_minutes{5};
  std::uint8_t cycles{4};

  bool valid() const;
};

enum class PomodoroStatus : std::uint8_t {
  Idle,
  Running,
  Paused,
  AwaitingTransition,
  Complete,
};

enum class PomodoroSegment : std::uint8_t {
  None,
  Focus,
  Break,
};

struct PomodoroSnapshot {
  PomodoroStatus status{PomodoroStatus::Idle};
  PomodoroSegment segment{PomodoroSegment::None};
  std::uint8_t current_cycle{1};
  std::uint32_t remaining_seconds{25 * 60};
  PomodoroPreset preset{};
  PomodoroPreset favourite{};
  std::uint32_t completed_sessions{0};
  std::uint64_t revision{0};
};

class PomodoroModel {
 public:
  PomodoroModel();

  bool configure(const PomodoroPreset& preset);
  bool save_favourite();
  bool load_favourite();
  bool start(std::uint32_t now_ms);
  bool pause(std::uint32_t now_ms);
  bool resume(std::uint32_t now_ms);
  bool skip(std::uint32_t now_ms);
  bool reset();
  bool confirm_transition(std::uint32_t now_ms);
  bool restore(const PomodoroSnapshot& stored);
  bool tick(std::uint32_t now_ms);

  PomodoroSnapshot snapshot() const;

 private:
  static std::uint32_t duration_ms(PomodoroSegment segment,
                                   const PomodoroPreset& preset);
  bool advance_segment(bool completed_naturally);
  void set_idle();
  void bump_revision();

  PomodoroPreset preset_{};
  PomodoroPreset favourite_{};
  PomodoroStatus status_{PomodoroStatus::Idle};
  PomodoroSegment segment_{PomodoroSegment::None};
  std::uint8_t current_cycle_{1};
  std::uint32_t remaining_ms_{25 * 60 * 1000};
  std::uint32_t last_tick_ms_{0};
  std::uint32_t completed_sessions_{0};
  std::uint64_t revision_{0};
};

}  // namespace kairos
