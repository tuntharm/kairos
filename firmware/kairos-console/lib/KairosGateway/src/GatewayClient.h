#pragma once

#include <Arduino.h>

#include "AppShell.h"
#include "PomodoroModule.h"

namespace kairos {

class GatewayClient {
 public:
  GatewayClient(AppShell& app, PomodoroModule& pomodoro);

  void begin();
  void loop(std::uint32_t now_ms);
  bool security_ready() const;
  void send_listen_requested();

 private:
  void poll_serial();
  bool run_security_self_test();

  AppShell& app_;
  PomodoroModule& pomodoro_;
  bool security_ready_{false};
  String serial_line_;
};

}  // namespace kairos
