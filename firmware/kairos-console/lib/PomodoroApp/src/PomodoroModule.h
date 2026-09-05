#pragma once

#include "AppModule.h"
#include "PomodoroModel.h"
#include "PomodoroStore.h"

namespace kairos {

class PomodoroModule : public AppModule {
 public:
  using ListenCallback = void (*)();

  bool begin();
  void set_listen_callback(ListenCallback callback);
  PomodoroModel& model();

  const char* id() const override;
  const char* title() const override;
  const char* subtitle() const override;
  std::uint64_t revision() const override;
  std::uint32_t character_xp() const override;
  void enter() override;
  void render(TFT_eSPI& display, bool full_redraw) override;
  bool on_touch(const TouchPoint& point, std::uint32_t now_ms) override;
  bool on_tick(std::uint32_t now_ms) override;
  void save_state() override;
  AppCommandDecision apply_command(const AppCommand& command,
                                   std::uint32_t now_ms) override;
  bool take_command_result(AppCommandResult& result) override;
  void cancel_pending_command() override;

 private:
  struct ParsedCommand {
    String message_id;
    String action;
    PomodoroPreset preset{};
    bool require_confirmation{false};
  };

  void draw_header(TFT_eSPI& display, const PomodoroSnapshot& snapshot);
  void draw_timer(TFT_eSPI& display, const PomodoroSnapshot& snapshot);
  void draw_setup(TFT_eSPI& display, const PomodoroSnapshot& snapshot);
  void draw_controls(TFT_eSPI& display, const PomodoroSnapshot& snapshot);
  void draw_transition(TFT_eSPI& display, const PomodoroSnapshot& snapshot);
  void draw_complete(TFT_eSPI& display, const PomodoroSnapshot& snapshot);
  void draw_confirmation(TFT_eSPI& display, const PomodoroSnapshot& snapshot);
  bool adjust_preset(int focus_delta, int break_delta, int cycle_delta);
  bool execute_command(const ParsedCommand& command, std::uint32_t now_ms);
  void finish_command(const ParsedCommand& command, bool changed, bool declined,
                      const char* message);
  void persist_after_action();
  void observe_phase_change(PomodoroStatus status, PomodoroStatus before);

  PomodoroModel model_;
  PomodoroStore store_;
  ListenCallback listen_callback_{nullptr};
  bool pending_confirmation_{false};
  ParsedCommand pending_command_{};
  bool has_command_result_{false};
  AppCommandResult command_result_{};
  std::uint32_t last_checkpoint_ms_{0};
  std::uint32_t last_rendered_seconds_{UINT32_MAX};
  std::uint64_t last_rendered_revision_{UINT64_MAX};
  std::uint8_t last_rendered_pulse_frame_{UINT8_MAX};
  bool pulse_active_{false};
  std::uint8_t pulse_frame_{0};
  std::uint32_t pulse_started_ms_{0};
};

}  // namespace kairos
