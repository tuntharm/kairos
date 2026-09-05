#include "AppShell.h"

#include <cstdio>
#include <cstring>

#include "CharacterProgress.h"
#include "HomeLayout.h"
#include "Theme.h"
#include "assets/mascot_base.hpp"

namespace kairos {

namespace {

String ellipsize(TFT_eSPI& display, String text, int maximum_width) {
  if (display.textWidth(text, 1) <= maximum_width) {
    return text;
  }
  while (!text.isEmpty()) {
    text.remove(text.length() - 1);
    String candidate = text;
    candidate += "...";
    if (display.textWidth(candidate, 1) <= maximum_width) {
      return candidate;
    }
  }
  return "...";
}

void split_status(TFT_eSPI& display, const String& message, String& first,
                  String& second) {
  constexpr int kMaximumWidth = 168;
  if (display.textWidth(message, 1) <= kMaximumWidth) {
    first = message;
    second = String();
    return;
  }

  std::size_t first_end = 0;
  std::size_t last_space = 0;
  for (std::size_t index = 0; index < message.length(); ++index) {
    const String candidate = message.substring(0, index + 1);
    if (display.textWidth(candidate, 1) > kMaximumWidth) {
      break;
    }
    first_end = index + 1;
    if (message[index] == ' ') {
      last_space = index;
    }
  }
  if (first_end == 0) {
    first_end = 1;
  }
  const std::size_t split = last_space > 0 ? last_space : first_end;
  first = ellipsize(display, message.substring(0, split), kMaximumWidth);
  first.trim();

  std::size_t start = split;
  while (start < message.length() && message[start] == ' ') {
    ++start;
  }
  second = message.substring(start);
  second.trim();
  second = ellipsize(display, second, kMaximumWidth);
}

}  // namespace

AppShell::AppShell(TFT_eSPI& display, AppModule* const* modules,
                   std::size_t module_count)
    : display_(display), modules_(modules), module_count_(module_count) {}

void AppShell::begin() {
  surface_ = Surface::Home;
  active_module_ = nullptr;
  home_scroll_ = clamp_home_scroll(home_scroll_, module_count_);
  dirty_ = true;
  draw_home();
}

void AppShell::tick(std::uint32_t now_ms) {
  if (status_active_ && !status_persistent_ &&
      static_cast<std::int32_t>(now_ms - status_expires_ms_) >= 0) {
    status_active_ = false;
    dirty_ = true;
  }
  bool active_changed = false;
  bool progress_changed = false;
  for (std::size_t index = 0; index < module_count_; ++index) {
    const bool changed = modules_[index]->on_tick(now_ms);
    if (modules_[index] == active_module_) {
      active_changed = changed;
    }
    if (changed && surface_ == Surface::Home) {
      progress_changed = true;
    }
  }
  if (surface_ == Surface::Home) {
    if (dirty_ || progress_changed) {
      draw_home();
    }
    return;
  }
  if (active_module_ != nullptr) {
    if (dirty_ || active_changed) {
      active_module_->render(display_, dirty_);
      if (status_active_) {
        draw_status();
      }
      dirty_ = false;
    }
  }
}

void AppShell::touch(const TouchPoint& point, std::uint32_t now_ms) {
  if (status_active_ && !status_persistent_) {
    clear_status();
  }

  if (surface_ == Surface::Home) {
    const HomeTouchResult hit =
        hit_home(point.x, point.y, module_count_, home_scroll_, status_active_);
    switch (hit.kind) {
      case HomeTouchResult::Kind::OpenModule:
        if (hit.module_index < module_count_) {
          open_module(modules_[hit.module_index]->id());
        }
        break;
      case HomeTouchResult::Kind::ScrollUp:
        if (home_scroll_ > 0) {
          home_scroll_ -= 1;
          dirty_ = true;
          draw_home();
        }
        break;
      case HomeTouchResult::Kind::ScrollDown:
        home_scroll_ = clamp_home_scroll(home_scroll_ + 1, module_count_);
        dirty_ = true;
        draw_home();
        break;
      case HomeTouchResult::Kind::None:
        break;
    }
    return;
  }

  if (home_inside(point.x, point.y, 0, 0, 54, 38)) {
    active_module_->save_state();
    surface_ = Surface::Home;
    active_module_ = nullptr;
    dirty_ = true;
    draw_home();
    return;
  }
  if (active_module_ != nullptr && active_module_->on_touch(point, now_ms)) {
    dirty_ = true;
  }
}

bool AppShell::open_module(const char* module_id) {
  AppModule* module = find_module(module_id);
  if (module == nullptr) {
    return false;
  }
  surface_ = Surface::Module;
  active_module_ = module;
  active_module_->enter();
  dirty_ = true;
  return true;
}

bool AppShell::module_revision(const char* module_id,
                               std::uint64_t& revision) const {
  AppModule* module = find_module(module_id);
  if (module == nullptr) {
    return false;
  }
  revision = module->revision();
  return true;
}

AppCommandDecision AppShell::apply_command(const char* module_id,
                                           const AppCommand& command,
                                           std::uint32_t now_ms) {
  AppModule* module = find_module(module_id);
  if (module == nullptr) {
    return {AppCommandError::UnsupportedAction, "unknown module"};
  }
  const AppCommandDecision decision = module->apply_command(command, now_ms);
  if (decision.accepted()) {
    open_module(module_id);
  }
  return decision;
}

bool AppShell::take_command_result(AppCommandResult& result) {
  for (std::size_t index = 0; index < module_count_; ++index) {
    if (modules_[index]->take_command_result(result)) {
      return true;
    }
  }
  return false;
}

void AppShell::cancel_pending_commands() {
  for (std::size_t index = 0; index < module_count_; ++index) {
    modules_[index]->cancel_pending_command();
  }
  dirty_ = true;
}

void AppShell::set_connection_state(bool connected) {
  if (connected_ != connected) {
    connected_ = connected;
    dirty_ = true;
  }
}

void AppShell::show_status(const String& message, StatusTone tone,
                           std::uint32_t now_ms, bool persistent) {
  status_message_ = message.substring(0, 80);
  status_tone_ = tone;
  status_active_ = !status_message_.isEmpty();
  status_persistent_ = persistent && status_active_;
  status_expires_ms_ = now_ms + 5000U;
  dirty_ = true;
}

void AppShell::clear_status() {
  status_message_ = String();
  status_active_ = false;
  status_persistent_ = false;
  dirty_ = true;
}

void AppShell::draw_home() {
  home_scroll_ = clamp_home_scroll(home_scroll_, module_count_);
  display_.fillScreen(theme::kBackground);
  display_.drawFastVLine(kRightPaneX - 1, 0, kHomeHeight, theme::kAccentDim);
  display_.drawFastVLine(kRightPaneX, 0, kHomeHeight, theme::kAccent);
  draw_character_pane();
  draw_app_list();
  draw_connection_badge();
  if (status_active_) {
    draw_status();
  }
  dirty_ = false;
}

void AppShell::draw_character_pane() {
  display_.pushImage(8, 4, kairos::assets::kMascotWidth,
                     kairos::assets::kMascotHeight, kairos::assets::kMascotBase);

  const CharacterProgress progress = character_progress(character_sessions());
  char rank_text[12];
  std::snprintf(rank_text, sizeof(rank_text), "RK %lu",
                static_cast<unsigned long>(progress.rank));
  char session_text[16];
  std::snprintf(session_text, sizeof(session_text), "%lu FOCUS",
                static_cast<unsigned long>(progress.sessions));

  display_.fillRect(4, 184, 120, 52, theme::kBackground);
  display_.setTextDatum(TL_DATUM);
  display_.setTextColor(theme::kAccent, theme::kBackground);
  display_.drawString(rank_text, 8, 186, 2);
  display_.setTextColor(theme::kMuted, theme::kBackground);
  display_.drawString(session_text, 8, 220, 1);

  constexpr int bar_x = 8;
  constexpr int bar_y = 206;
  constexpr int segment_w = 24;
  for (std::uint32_t index = 0; index < kSessionsPerRank; ++index) {
    const int x = bar_x + static_cast<int>(index) * (segment_w + 3);
    display_.drawRect(x, bar_y, segment_w, 8, theme::kAccentDim);
    if (index < progress.xp_in_rank) {
      display_.fillRect(x + 1, bar_y + 1, segment_w - 2, 6, theme::kAccent);
    }
  }
}

void AppShell::draw_app_list() {
  display_.setTextDatum(TL_DATUM);
  display_.setTextColor(theme::kAccent, theme::kBackground);
  display_.drawString("KAIROS", kAppListX, 8, 2);

  const std::size_t visible =
      module_count_ < static_cast<std::size_t>(kVisibleAppRows)
          ? module_count_
          : static_cast<std::size_t>(kVisibleAppRows);
  for (std::size_t row = 0; row < visible; ++row) {
    const std::size_t index = home_scroll_ + row;
    const int y = kAppListY + static_cast<int>(row) * kAppRowHeight;
    display_.fillRoundRect(kAppListX, y, kAppListWidth, kAppRowHeight - 6, 4,
                           theme::kPanel);
    display_.drawRoundRect(kAppListX, y, kAppListWidth, kAppRowHeight - 6, 4,
                           theme::kAccent);
    display_.setTextColor(theme::kText, theme::kPanel);
    display_.drawString(modules_[index]->title(), kAppListX + 10, y + 8, 2);
  }

  if (max_home_scroll(module_count_) > 0) {
    display_.setTextDatum(TC_DATUM);
    display_.setTextColor(home_scroll_ > 0 ? theme::kAccent : theme::kMuted,
                          theme::kBackground);
    display_.drawString("^", kChevronX + 8, kAppListY + 4, 2);
    display_.setTextColor(home_scroll_ < max_home_scroll(module_count_)
                              ? theme::kAccent
                              : theme::kMuted,
                          theme::kBackground);
    display_.drawString("v", kChevronX + 8, kAppListY + 112, 2);
    display_.setTextDatum(TL_DATUM);
  }
}

void AppShell::draw_connection_badge() {
  display_.setTextDatum(TR_DATUM);
  display_.setTextColor(connected_ ? theme::kAccent : theme::kMuted,
                        theme::kBackground);
  display_.drawString(connected_ ? "LINK" : "LOCAL", 312, 10, 1);
  display_.setTextDatum(TL_DATUM);
}

void AppShell::draw_status() {
  const std::uint16_t accent = status_tone_ == StatusTone::Offline
                                   ? theme::kOffline
                                   : (status_tone_ == StatusTone::Success
                                          ? theme::kAccent
                                          : theme::kAmber);
  display_.fillRoundRect(kStatusX, kStatusY, kStatusWidth, kStatusHeight, 5,
                         theme::kPanel);
  display_.drawRoundRect(kStatusX, kStatusY, kStatusWidth, kStatusHeight, 5,
                         accent);
  String first;
  String second;
  split_status(display_, status_message_, first, second);
  display_.setTextDatum(TC_DATUM);
  display_.setTextColor(accent, theme::kPanel);
  if (second.isEmpty()) {
    display_.drawString(first, kStatusX + kStatusWidth / 2, kStatusY + 24, 1);
  } else {
    display_.drawString(first, kStatusX + kStatusWidth / 2, kStatusY + 16, 1);
    display_.drawString(second, kStatusX + kStatusWidth / 2, kStatusY + 34, 1);
  }
  display_.setTextDatum(TL_DATUM);
}

std::uint32_t AppShell::character_sessions() const {
  std::uint32_t sessions = 0;
  for (std::size_t index = 0; index < module_count_; ++index) {
    sessions += modules_[index]->character_xp();
  }
  return sessions;
}

AppModule* AppShell::find_module(const char* module_id) const {
  if (module_id == nullptr) {
    return nullptr;
  }
  for (std::size_t index = 0; index < module_count_; ++index) {
    if (std::strcmp(modules_[index]->id(), module_id) == 0) {
      return modules_[index];
    }
  }
  return nullptr;
}

}  // namespace kairos
