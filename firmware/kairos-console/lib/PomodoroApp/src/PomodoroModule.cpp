#include "PomodoroModule.h"

#include <cstdio>
#include <cstring>
#include <initializer_list>

#include "Theme.h"

namespace kairos {

namespace {

constexpr std::uint32_t kCheckpointIntervalMs = 60U * 1000U;
constexpr std::uint32_t kPulseFrameMs = 70U;
constexpr std::uint8_t kPulseFrameCount = 4;

bool inside(const TouchPoint& point, int x, int y, int width, int height) {
  return point.x >= x && point.x < x + width && point.y >= y &&
         point.y < y + height;
}

bool has_exact_keys(JsonObjectConst object,
                    std::initializer_list<const char*> keys) {
  if (object.size() != keys.size()) {
    return false;
  }
  for (JsonPairConst pair : object) {
    bool expected = false;
    for (const char* key : keys) {
      if (std::strcmp(pair.key().c_str(), key) == 0) {
        expected = true;
        break;
      }
    }
    if (!expected) {
      return false;
    }
  }
  return true;
}

const char* status_text(PomodoroStatus status) {
  switch (status) {
    case PomodoroStatus::Idle:
      return "READY";
    case PomodoroStatus::Running:
      return "RUNNING";
    case PomodoroStatus::Paused:
      return "PAUSED";
    case PomodoroStatus::AwaitingTransition:
      return "AWAITING";
    case PomodoroStatus::Complete:
      return "COMPLETE";
  }
  return "UNKNOWN";
}

const char* segment_text(PomodoroSegment segment) {
  switch (segment) {
    case PomodoroSegment::Focus:
      return "FOCUS";
    case PomodoroSegment::Break:
      return "BREAK";
    case PomodoroSegment::None:
      return "SESSION";
  }
  return "SESSION";
}

}  // namespace

bool PomodoroModule::begin() {
  if (!store_.begin()) {
    return false;
  }
  store_.load(model_);
  return true;
}

void PomodoroModule::set_listen_callback(ListenCallback callback) {
  listen_callback_ = callback;
}

PomodoroModel& PomodoroModule::model() { return model_; }

const char* PomodoroModule::id() const { return "pomodoro"; }

const char* PomodoroModule::title() const { return "POMODORO"; }

const char* PomodoroModule::subtitle() const { return "FOCUS TIMER // READY"; }

std::uint64_t PomodoroModule::revision() const {
  return model_.snapshot().revision;
}

std::uint32_t PomodoroModule::character_xp() const {
  return model_.snapshot().completed_sessions;
}

void PomodoroModule::enter() {
  last_rendered_seconds_ = UINT32_MAX;
  last_rendered_revision_ = UINT64_MAX;
  last_rendered_pulse_frame_ = UINT8_MAX;
}

void PomodoroModule::render(TFT_eSPI& display, bool full_redraw) {
  const PomodoroSnapshot snapshot = model_.snapshot();
  const std::uint8_t rendered_pulse_frame =
      pulse_active_ ? pulse_frame_ : UINT8_MAX;
  if (full_redraw || snapshot.revision != last_rendered_revision_) {
    display.fillScreen(theme::kBackground);
    draw_header(display, snapshot);
    if (pending_confirmation_) {
      draw_confirmation(display, snapshot);
      last_rendered_seconds_ = snapshot.remaining_seconds;
      last_rendered_revision_ = snapshot.revision;
      last_rendered_pulse_frame_ = rendered_pulse_frame;
      return;
    }
    switch (snapshot.status) {
      case PomodoroStatus::Idle:
        draw_setup(display, snapshot);
        break;
      case PomodoroStatus::Running:
      case PomodoroStatus::Paused:
        draw_timer(display, snapshot);
        draw_controls(display, snapshot);
        break;
      case PomodoroStatus::AwaitingTransition:
        draw_transition(display, snapshot);
        break;
      case PomodoroStatus::Complete:
        draw_complete(display, snapshot);
        break;
    }
  } else if (!pending_confirmation_ &&
             snapshot.status == PomodoroStatus::AwaitingTransition &&
             rendered_pulse_frame != last_rendered_pulse_frame_) {
    draw_transition(display, snapshot);
  } else if (!pending_confirmation_ &&
             snapshot.status == PomodoroStatus::Running &&
             snapshot.remaining_seconds != last_rendered_seconds_) {
    draw_timer(display, snapshot);
  }
  last_rendered_seconds_ = snapshot.remaining_seconds;
  last_rendered_revision_ = snapshot.revision;
  last_rendered_pulse_frame_ = rendered_pulse_frame;
}

bool PomodoroModule::on_touch(const TouchPoint& point, std::uint32_t now_ms) {
  const PomodoroSnapshot before = model_.snapshot();
  if (inside(point, 270, 4, 46, 38)) {
    if (listen_callback_ != nullptr) {
      listen_callback_();
    }
    return true;
  }

  if (pending_confirmation_) {
    if (inside(point, 24, 174, 128, 52)) {
      const ParsedCommand command = pending_command_;
      pending_confirmation_ = false;
      const bool changed = execute_command(command, now_ms);
      finish_command(
          command, changed, false,
          changed ? "confirmed and applied" : "command is invalid in current state");
      if (model_.snapshot().revision != before.revision) {
        persist_after_action();
      }
      return true;
    }
    if (inside(point, 168, 174, 128, 52)) {
      const ParsedCommand command = pending_command_;
      pending_confirmation_ = false;
      finish_command(command, false, true, "cancelled on device");
      return true;
    }
    return false;
  }

  bool changed = false;
  switch (before.status) {
    case PomodoroStatus::Idle:
      if (inside(point, 14, 78, 88, 42)) {
        changed = adjust_preset(1, 0, 0);
      } else if (inside(point, 14, 121, 88, 42)) {
        changed = adjust_preset(-1, 0, 0);
      } else if (inside(point, 116, 78, 88, 42)) {
        changed = adjust_preset(0, 1, 0);
      } else if (inside(point, 116, 121, 88, 42)) {
        changed = adjust_preset(0, -1, 0);
      } else if (inside(point, 218, 78, 88, 42)) {
        changed = adjust_preset(0, 0, 1);
      } else if (inside(point, 218, 121, 88, 42)) {
        changed = adjust_preset(0, 0, -1);
      } else if (inside(point, 12, 190, 72, 40)) {
        changed = model_.save_favourite();
      } else if (inside(point, 90, 190, 72, 40)) {
        changed = model_.load_favourite();
      } else if (inside(point, 168, 190, 140, 40)) {
        changed = model_.start(now_ms);
      }
      break;
    case PomodoroStatus::Running:
      if (inside(point, 18, 184, 102, 44)) {
        changed = model_.pause(now_ms);
      } else if (inside(point, 128, 184, 82, 44)) {
        changed = model_.skip(now_ms);
      } else if (inside(point, 218, 184, 84, 44)) {
        changed = model_.reset();
      }
      break;
    case PomodoroStatus::Paused:
      if (inside(point, 18, 184, 102, 44)) {
        changed = model_.resume(now_ms);
      } else if (inside(point, 128, 184, 82, 44)) {
        changed = model_.skip(now_ms);
      } else if (inside(point, 218, 184, 84, 44)) {
        changed = model_.reset();
      }
      break;
    case PomodoroStatus::AwaitingTransition:
      if (inside(point, 34, 76, 252, 102)) {
        changed = model_.confirm_transition(now_ms);
      } else if (inside(point, 218, 184, 84, 44)) {
        changed = model_.reset();
      }
      break;
    case PomodoroStatus::Complete:
      if (inside(point, 88, 184, 144, 44)) {
        changed = model_.reset();
      }
      break;
  }
  if (changed) {
    persist_after_action();
  }
  return changed;
}

bool PomodoroModule::on_tick(std::uint32_t now_ms) {
  const PomodoroSnapshot before = model_.snapshot();
  model_.tick(now_ms);
  const PomodoroSnapshot after = model_.snapshot();
  if (pulse_active_) {
    const std::uint8_t frame = static_cast<std::uint8_t>(
        ((now_ms - pulse_started_ms_) / kPulseFrameMs) % kPulseFrameCount);
    if (frame != pulse_frame_) {
      pulse_frame_ = frame;
    }
    if (now_ms - pulse_started_ms_ > 800U) {
      pulse_active_ = false;
    }
  }
  if (after.status == PomodoroStatus::Running &&
      now_ms - last_checkpoint_ms_ >= kCheckpointIntervalMs) {
    persist_after_action();
    last_checkpoint_ms_ = now_ms;
  }
  observe_phase_change(after.status, before.status);
  return after.revision != before.revision || pulse_active_;
}

void PomodoroModule::save_state() { store_.save(model_.snapshot()); }

void PomodoroModule::draw_header(TFT_eSPI& display,
                                 const PomodoroSnapshot& snapshot) {
  display.setTextDatum(TL_DATUM);
  display.setTextColor(theme::kMuted, theme::kBackground);
  display.drawString("< HOME", 8, 13, 2);
  display.setTextColor(theme::kAccent, theme::kBackground);
  display.drawString("POMODORO", 70, 13, 2);
  display.setTextColor(snapshot.status == PomodoroStatus::Paused ? theme::kDanger
                                                                : theme::kAccent,
                       theme::kBackground);
  display.drawString(status_text(snapshot.status), 264, 13, 1);
  display.drawRoundRect(270, 8, 42, 22, 3, theme::kAccentDim);
  display.setTextColor(theme::kMuted, theme::kBackground);
  display.drawString("MIC", 281, 15, 1);
}

void PomodoroModule::draw_timer(TFT_eSPI& display,
                                const PomodoroSnapshot& snapshot) {
  display.fillRect(10, 52, 300, 122, theme::kBackground);
  char time_text[8];
  const std::uint32_t minutes = snapshot.remaining_seconds / 60U;
  const std::uint32_t seconds = snapshot.remaining_seconds % 60U;
  std::snprintf(time_text, sizeof(time_text), "%02lu:%02lu",
                static_cast<unsigned long>(minutes),
                static_cast<unsigned long>(seconds));
  const char* phase_text = segment_text(snapshot.segment);
  display.setTextDatum(TC_DATUM);
  display.setTextColor(theme::kMuted, theme::kBackground);
  display.drawString(phase_text, 160, 58, 2);
  display.setTextColor(theme::kAccent, theme::kBackground);
  display.drawString(time_text, 160, 82, 7);

  const std::uint32_t total_seconds =
      60U * (snapshot.segment == PomodoroSegment::Break
                 ? snapshot.preset.break_minutes
                 : snapshot.preset.focus_minutes);
  int progress = 0;
  if (total_seconds > 0) {
    progress = static_cast<int>((280U * (total_seconds - snapshot.remaining_seconds)) /
                                total_seconds);
  }
  display.drawRoundRect(18, 152, 284, 14, 4, theme::kAccentDim);
  if (progress > 0) {
    display.fillRoundRect(20, 154, progress, 10, 4, theme::kAccent);
  }
  display.setTextDatum(TL_DATUM);
}

void PomodoroModule::draw_setup(TFT_eSPI& display,
                                const PomodoroSnapshot& snapshot) {
  const int x_positions[] = {14, 116, 218};
  const char* labels[] = {"FOCUS", "BREAK", "CYCLES"};
  const int values[] = {snapshot.preset.focus_minutes,
                        snapshot.preset.break_minutes, snapshot.preset.cycles};
  for (int index = 0; index < 3; ++index) {
    display.fillRoundRect(x_positions[index], 52, 88, 128, 8, theme::kPanel);
    display.drawRoundRect(x_positions[index], 52, 88, 128, 8, theme::kAccentDim);
    display.setTextDatum(TC_DATUM);
    display.setTextColor(theme::kMuted, theme::kPanel);
    display.drawString(labels[index], x_positions[index] + 44, 56, 1);
    display.setTextColor(theme::kAccent, theme::kPanel);
    display.drawString("+", x_positions[index] + 44, 86, 4);
    char value[8];
    std::snprintf(value, sizeof(value), "%d", values[index]);
    display.setTextColor(theme::kText, theme::kPanel);
    display.drawString(value, x_positions[index] + 44, 112, 4);
    display.setTextColor(theme::kAccent, theme::kPanel);
    display.drawString("-", x_positions[index] + 44, 137, 4);
  }

  char favourite[28];
  std::snprintf(favourite, sizeof(favourite), "FAV %u/%u x%u",
                snapshot.favourite.focus_minutes, snapshot.favourite.break_minutes,
                snapshot.favourite.cycles);
  display.setTextDatum(TC_DATUM);
  display.setTextColor(theme::kMuted, theme::kBackground);
  display.drawString(favourite, 160, 169, 1);

  display.drawRoundRect(12, 190, 72, 40, 7, theme::kAccent);
  display.setTextColor(theme::kAccent, theme::kBackground);
  display.drawString("SAVE", 48, 210, 2);
  display.fillRoundRect(90, 190, 72, 40, 7, theme::kPanel);
  display.setTextColor(theme::kAccent, theme::kPanel);
  display.drawString("LOAD", 126, 210, 2);
  display.fillRoundRect(168, 190, 140, 40, 7, theme::kAccent);
  display.setTextColor(theme::kBackground, theme::kAccent);
  display.drawString("START", 238, 210, 2);
  display.setTextDatum(TL_DATUM);
}

void PomodoroModule::draw_controls(TFT_eSPI& display,
                                   const PomodoroSnapshot& snapshot) {
  display.fillRoundRect(18, 184, 102, 44, 7, theme::kAccent);
  display.setTextDatum(TC_DATUM);
  display.setTextColor(theme::kBackground, theme::kAccent);
  display.drawString(snapshot.status == PomodoroStatus::Paused ? "RESUME" : "PAUSE",
                     69, 206, 2);
  display.drawRoundRect(128, 184, 82, 44, 7, theme::kAccent);
  display.setTextColor(theme::kAccent, theme::kBackground);
  display.drawString("SKIP", 169, 206, 2);
  display.drawRoundRect(218, 184, 84, 44, 7, theme::kDanger);
  display.setTextColor(theme::kDanger, theme::kBackground);
  display.drawString("RESET", 260, 206, 2);
  display.setTextDatum(TL_DATUM);
}

void PomodoroModule::draw_transition(TFT_eSPI& display,
                                     const PomodoroSnapshot& snapshot) {
  const int inset = pulse_active_ ? pulse_frame_ : 0;
  display.fillRoundRect(34 + inset, 76 + inset, 252 - inset * 2,
                        102 - inset * 2, 7, theme::kPanel);
  display.drawRoundRect(34, 76, 252, 102, 8, theme::kAccent);
  display.setTextDatum(TC_DATUM);
  display.setTextColor(theme::kAccent, theme::kPanel);
  display.drawString("NEXT SEGMENT", 160, 91, 2);
  display.setTextColor(theme::kText, theme::kPanel);
  display.drawString(segment_text(snapshot.segment), 160, 116, 4);
  display.setTextColor(theme::kMuted, theme::kPanel);
  display.drawString("TAP TO BEGIN", 160, 151, 2);
  display.drawRoundRect(218, 184, 84, 44, 7, theme::kDanger);
  display.setTextColor(theme::kDanger, theme::kBackground);
  display.drawString("RESET", 260, 206, 2);
  display.setTextDatum(TL_DATUM);
}

void PomodoroModule::draw_complete(TFT_eSPI& display,
                                   const PomodoroSnapshot& snapshot) {
  display.setTextDatum(TC_DATUM);
  display.setTextColor(theme::kAccent, theme::kBackground);
  display.drawString("SEQUENCE COMPLETE", 160, 76, 2);
  char completed[28];
  std::snprintf(completed, sizeof(completed), "%lu FOCUS SESSIONS",
                static_cast<unsigned long>(snapshot.completed_sessions));
  display.setTextColor(theme::kText, theme::kBackground);
  display.drawString(completed, 160, 114, 4);
  display.fillRoundRect(88, 184, 144, 44, 7, theme::kAccent);
  display.setTextColor(theme::kBackground, theme::kAccent);
  display.drawString("RESET", 160, 206, 2);
  display.setTextDatum(TL_DATUM);
}

void PomodoroModule::draw_confirmation(TFT_eSPI& display,
                                       const PomodoroSnapshot& snapshot) {
  display.drawRoundRect(18, 58, 284, 102, 8, theme::kAccent);
  display.fillRoundRect(20, 60, 280, 98, 7, theme::kPanel);
  display.setTextDatum(TC_DATUM);
  display.setTextColor(theme::kAccent, theme::kPanel);
  display.drawString("CONFIRM REMOTE COMMAND", 160, 70, 2);
  if (pending_command_.action == "reset") {
    display.setTextColor(theme::kText, theme::kPanel);
    display.drawString("RESET CURRENT SESSION?", 160, 108, 2);
  } else {
    char preset[20];
    std::snprintf(preset, sizeof(preset), "%u-%u x%u",
                  pending_command_.preset.focus_minutes,
                  pending_command_.preset.break_minutes,
                  pending_command_.preset.cycles);
    display.setTextColor(theme::kText, theme::kPanel);
    display.drawString("POMODORO", 160, 98, 2);
    display.drawString(preset, 160, 122, 4);
  }
  (void)snapshot;
  display.fillRoundRect(24, 174, 128, 52, 7, theme::kAccent);
  display.setTextColor(theme::kBackground, theme::kAccent);
  display.drawString("CONFIRM", 88, 200, 2);
  display.drawRoundRect(168, 174, 128, 52, 7, theme::kDanger);
  display.setTextColor(theme::kDanger, theme::kBackground);
  display.drawString("CANCEL", 232, 200, 2);
  display.setTextDatum(TL_DATUM);
}

bool PomodoroModule::adjust_preset(int focus_delta, int break_delta,
                                   int cycle_delta) {
  PomodoroPreset preset = model_.snapshot().preset;
  const int focus = static_cast<int>(preset.focus_minutes) + focus_delta;
  const int brk = static_cast<int>(preset.break_minutes) + break_delta;
  const int cycles = static_cast<int>(preset.cycles) + cycle_delta;
  preset.focus_minutes = static_cast<std::uint16_t>(focus);
  preset.break_minutes = static_cast<std::uint16_t>(brk);
  preset.cycles = static_cast<std::uint8_t>(cycles);
  return model_.configure(preset);
}

AppCommandDecision PomodoroModule::apply_command(const AppCommand& command,
                                                 std::uint32_t now_ms) {
  if (command.action != "configure" && command.action != "reset") {
    return {AppCommandError::UnsupportedAction, "unknown Pomodoro command"};
  }
  ParsedCommand parsed;
  parsed.message_id = command.message_id;
  parsed.action = command.action;
  parsed.require_confirmation = command.payload["requireConfirmation"] | true;
  if (command.action == "configure") {
    parsed.preset.focus_minutes = command.payload["focusMinutes"] | 25;
    parsed.preset.break_minutes = command.payload["breakMinutes"] | 5;
    parsed.preset.cycles = command.payload["cycles"] | 4;
    if (!parsed.preset.valid()) {
      return {AppCommandError::InvalidPayload, "invalid preset"};
    }
  }
  (void)has_exact_keys;
  (void)now_ms;
  if (parsed.require_confirmation) {
    pending_confirmation_ = true;
    pending_command_ = parsed;
    return {};
  }
  if (!execute_command(parsed, now_ms)) {
    return {AppCommandError::Busy, "command is invalid in current state"};
  }
  persist_after_action();
  return {};
}

bool PomodoroModule::take_command_result(AppCommandResult& result) {
  if (!has_command_result_) {
    return false;
  }
  result = command_result_;
  has_command_result_ = false;
  return true;
}

void PomodoroModule::cancel_pending_command() {
  if (pending_confirmation_) {
    pending_confirmation_ = false;
    finish_command(pending_command_, false, true, "cancelled on device");
  }
}

bool PomodoroModule::execute_command(const ParsedCommand& command,
                                     std::uint32_t now_ms) {
  if (command.action == "reset") {
    return model_.reset();
  }
  if (command.action == "configure") {
    if (model_.snapshot().status != PomodoroStatus::Idle &&
        model_.snapshot().status != PomodoroStatus::Complete) {
      return false;
    }
    return model_.configure(command.preset);
  }
  return false;
}

void PomodoroModule::finish_command(const ParsedCommand& command, bool changed,
                                    bool declined, const char* message) {
  command_result_.message_id = command.message_id;
  command_result_.success = changed;
  command_result_.confirmation_declined = declined;
  command_result_.message = message;
  command_result_.revision = model_.snapshot().revision;
  has_command_result_ = true;
}

void PomodoroModule::persist_after_action() { store_.save(model_.snapshot()); }

void PomodoroModule::observe_phase_change(PomodoroStatus status,
                                          PomodoroStatus before) {
  if (status == PomodoroStatus::AwaitingTransition &&
      before != PomodoroStatus::AwaitingTransition) {
    pulse_active_ = true;
    pulse_frame_ = 0;
    pulse_started_ms_ = millis();
  }
}

}  // namespace kairos
