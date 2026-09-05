#include "PomodoroModel.h"

namespace kairos {

namespace {

bool snapshot_shape_is_valid(const PomodoroSnapshot& snapshot) {
  if (!snapshot.preset.valid() || !snapshot.favourite.valid()) {
    return false;
  }
  switch (snapshot.status) {
    case PomodoroStatus::Idle:
    case PomodoroStatus::Complete:
      return snapshot.segment == PomodoroSegment::None;
    case PomodoroStatus::Running:
    case PomodoroStatus::Paused:
    case PomodoroStatus::AwaitingTransition:
      return snapshot.segment != PomodoroSegment::None;
  }
  return false;
}

}  // namespace

bool PomodoroPreset::valid() const {
  return focus_minutes >= 1 && focus_minutes <= 90 && break_minutes >= 1 &&
         break_minutes <= 30 && cycles >= 1 && cycles <= 8;
}

PomodoroModel::PomodoroModel() { set_idle(); }

bool PomodoroModel::configure(const PomodoroPreset& preset) {
  if (!preset.valid() ||
      (status_ != PomodoroStatus::Idle && status_ != PomodoroStatus::Complete)) {
    return false;
  }
  preset_ = preset;
  remaining_ms_ = duration_ms(PomodoroSegment::Focus, preset_);
  status_ = PomodoroStatus::Idle;
  segment_ = PomodoroSegment::None;
  current_cycle_ = 1;
  bump_revision();
  return true;
}

bool PomodoroModel::save_favourite() {
  if (status_ != PomodoroStatus::Idle && status_ != PomodoroStatus::Complete) {
    return false;
  }
  favourite_ = preset_;
  bump_revision();
  return true;
}

bool PomodoroModel::load_favourite() { return configure(favourite_); }

bool PomodoroModel::start(std::uint32_t now_ms) {
  if (status_ != PomodoroStatus::Idle) {
    return false;
  }
  status_ = PomodoroStatus::Running;
  segment_ = PomodoroSegment::Focus;
  current_cycle_ = 1;
  remaining_ms_ = duration_ms(PomodoroSegment::Focus, preset_);
  last_tick_ms_ = now_ms;
  bump_revision();
  return true;
}

bool PomodoroModel::pause(std::uint32_t now_ms) {
  if (status_ != PomodoroStatus::Running) {
    return false;
  }
  tick(now_ms);
  if (status_ != PomodoroStatus::Running) {
    return false;
  }
  status_ = PomodoroStatus::Paused;
  bump_revision();
  return true;
}

bool PomodoroModel::resume(std::uint32_t now_ms) {
  if (status_ != PomodoroStatus::Paused) {
    return false;
  }
  status_ = PomodoroStatus::Running;
  last_tick_ms_ = now_ms;
  bump_revision();
  return true;
}

bool PomodoroModel::skip(std::uint32_t now_ms) {
  if (status_ != PomodoroStatus::Running && status_ != PomodoroStatus::Paused &&
      status_ != PomodoroStatus::AwaitingTransition) {
    return false;
  }
  if (status_ == PomodoroStatus::Running) {
    tick(now_ms);
    if (status_ != PomodoroStatus::Running) {
      return true;
    }
  }
  return advance_segment(false);
}

bool PomodoroModel::reset() {
  if (status_ == PomodoroStatus::Idle) {
    return false;
  }
  set_idle();
  bump_revision();
  return true;
}

bool PomodoroModel::confirm_transition(std::uint32_t now_ms) {
  if (status_ != PomodoroStatus::AwaitingTransition) {
    return false;
  }
  status_ = PomodoroStatus::Running;
  remaining_ms_ = duration_ms(segment_, preset_);
  last_tick_ms_ = now_ms;
  bump_revision();
  return true;
}

bool PomodoroModel::restore(const PomodoroSnapshot& stored) {
  if (!snapshot_shape_is_valid(stored)) {
    return false;
  }
  preset_ = stored.preset;
  favourite_ = stored.favourite;
  remaining_ms_ = stored.remaining_seconds * 1000U;
  if (remaining_ms_ > duration_ms(stored.segment == PomodoroSegment::None
                                      ? PomodoroSegment::Focus
                                      : stored.segment,
                                  stored.preset)) {
    remaining_ms_ = duration_ms(stored.segment == PomodoroSegment::None
                                    ? PomodoroSegment::Focus
                                    : stored.segment,
                                stored.preset);
  }
  current_cycle_ = stored.current_cycle == 0 ? 1 : stored.current_cycle;
  completed_sessions_ = stored.completed_sessions;
  status_ = stored.status == PomodoroStatus::Running ? PomodoroStatus::Paused
                                                     : stored.status;
  segment_ = stored.segment;
  last_tick_ms_ = 0;
  bump_revision();
  return true;
}

bool PomodoroModel::tick(std::uint32_t now_ms) {
  if (status_ != PomodoroStatus::Running) {
    return false;
  }
  const std::uint32_t elapsed = now_ms - last_tick_ms_;
  last_tick_ms_ = now_ms;
  if (elapsed >= remaining_ms_) {
    remaining_ms_ = 0;
    return advance_segment(true);
  }
  remaining_ms_ -= elapsed;
  return false;
}

PomodoroSnapshot PomodoroModel::snapshot() const {
  return PomodoroSnapshot{
      status_,
      segment_,
      current_cycle_,
      remaining_ms_ / 1000U,
      preset_,
      favourite_,
      completed_sessions_,
      revision_,
  };
}

std::uint32_t PomodoroModel::duration_ms(PomodoroSegment segment,
                                         const PomodoroPreset& preset) {
  const std::uint16_t minutes =
      segment == PomodoroSegment::Break ? preset.break_minutes
                                        : preset.focus_minutes;
  return static_cast<std::uint32_t>(minutes) * 60U * 1000U;
}

bool PomodoroModel::advance_segment(bool completed_naturally) {
  if (segment_ == PomodoroSegment::Focus) {
    if (completed_naturally) {
      completed_sessions_ += 1;
    }
    if (current_cycle_ >= preset_.cycles) {
      status_ = PomodoroStatus::Complete;
      segment_ = PomodoroSegment::None;
      remaining_ms_ = 0;
    } else {
      status_ = PomodoroStatus::AwaitingTransition;
      segment_ = PomodoroSegment::Break;
      remaining_ms_ = duration_ms(PomodoroSegment::Break, preset_);
    }
  } else if (segment_ == PomodoroSegment::Break) {
    current_cycle_ = static_cast<std::uint8_t>(current_cycle_ + 1);
    status_ = PomodoroStatus::AwaitingTransition;
    segment_ = PomodoroSegment::Focus;
    remaining_ms_ = duration_ms(PomodoroSegment::Focus, preset_);
  } else {
    return false;
  }
  bump_revision();
  return true;
}

void PomodoroModel::set_idle() {
  status_ = PomodoroStatus::Idle;
  segment_ = PomodoroSegment::None;
  current_cycle_ = 1;
  remaining_ms_ = duration_ms(PomodoroSegment::Focus, preset_);
  last_tick_ms_ = 0;
}

void PomodoroModel::bump_revision() { revision_ += 1; }

}  // namespace kairos
