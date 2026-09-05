#pragma once

#include "AppModule.h"

namespace kairos {

class WardrobeModule : public AppModule {
 public:
  const char* id() const override;
  const char* title() const override;
  const char* subtitle() const override;
  std::uint64_t revision() const override;
  void enter() override;
  void render(TFT_eSPI& display, bool full_redraw) override;
  bool on_touch(const TouchPoint& point, std::uint32_t now_ms) override;
  bool on_tick(std::uint32_t now_ms) override;
  void save_state() override;
  AppCommandDecision apply_command(const AppCommand& command,
                                   std::uint32_t now_ms) override;
  bool take_command_result(AppCommandResult& result) override;
  void cancel_pending_command() override;
};

}  // namespace kairos
