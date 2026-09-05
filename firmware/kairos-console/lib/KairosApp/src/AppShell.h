#pragma once

#include <Arduino.h>
#include <TFT_eSPI.h>

#include "AppModule.h"

namespace kairos {

enum class StatusTone : std::uint8_t { Info, Success, Offline };

class AppShell {
 public:
  AppShell(TFT_eSPI& display, AppModule* const* modules,
           std::size_t module_count);

  void begin();
  void tick(std::uint32_t now_ms);
  void touch(const TouchPoint& point, std::uint32_t now_ms);
  bool open_module(const char* module_id);
  bool module_revision(const char* module_id, std::uint64_t& revision) const;
  AppCommandDecision apply_command(const char* module_id,
                                   const AppCommand& command,
                                   std::uint32_t now_ms);
  bool take_command_result(AppCommandResult& result);
  void cancel_pending_commands();
  void set_connection_state(bool connected);
  void show_status(const String& message, StatusTone tone,
                   std::uint32_t now_ms, bool persistent = false);
  void clear_status();

 private:
  enum class Surface : std::uint8_t { Home, Module };

  void draw_home();
  void draw_character_pane();
  void draw_app_list();
  void draw_connection_badge();
  void draw_status();
  std::uint32_t character_sessions() const;
  AppModule* find_module(const char* module_id) const;

  TFT_eSPI& display_;
  AppModule* const* modules_;
  std::size_t module_count_;
  AppModule* active_module_{nullptr};
  Surface surface_{Surface::Home};
  bool connected_{false};
  bool dirty_{true};
  bool status_active_{false};
  bool status_persistent_{false};
  String status_message_;
  StatusTone status_tone_{StatusTone::Info};
  std::uint32_t status_expires_ms_{0};
  std::size_t home_scroll_{0};
};

}  // namespace kairos
