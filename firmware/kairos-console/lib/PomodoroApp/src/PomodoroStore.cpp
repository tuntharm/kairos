#include "PomodoroStore.h"

#include <Preferences.h>
#include <cstddef>
#include <cstring>

namespace kairos {

namespace {

constexpr char kNamespace[] = "kairos-pomo";
constexpr char kBlobKey[] = "state";

struct PersistedState {
  std::uint32_t magic;
  std::uint16_t version;
  std::uint8_t status;
  std::uint8_t segment;
  std::uint8_t current_cycle;
  std::uint8_t cycles;
  std::uint16_t focus_minutes;
  std::uint16_t break_minutes;
  std::uint16_t favourite_focus;
  std::uint16_t favourite_break;
  std::uint8_t favourite_cycles;
  std::uint32_t remaining_seconds;
  std::uint32_t completed_sessions;
  std::uint32_t checksum;
};

constexpr std::uint32_t kMagic = 0x504F4D31;
constexpr std::uint16_t kVersion = 1;

std::uint32_t checksum(const PersistedState& state) {
  const auto* bytes = reinterpret_cast<const std::uint8_t*>(&state);
  std::uint32_t sum = 2166136261U;
  for (std::size_t index = 0; index < offsetof(PersistedState, checksum);
       ++index) {
    sum ^= bytes[index];
    sum *= 16777619U;
  }
  return sum;
}

}  // namespace

bool PomodoroStore::begin() {
  Preferences preferences;
  if (!preferences.begin(kNamespace, false)) {
    return false;
  }
  preferences.end();
  return true;
}

bool PomodoroStore::load(PomodoroModel& model) {
  Preferences preferences;
  if (!preferences.begin(kNamespace, true)) {
    return false;
  }
  PersistedState persisted{};
  const size_t read =
      preferences.getBytes(kBlobKey, &persisted, sizeof(persisted));
  preferences.end();
  if (read != sizeof(persisted) || persisted.magic != kMagic ||
      persisted.version != kVersion || persisted.checksum != checksum(persisted)) {
    return false;
  }

  PomodoroSnapshot snapshot;
  snapshot.status = static_cast<PomodoroStatus>(persisted.status);
  snapshot.segment = static_cast<PomodoroSegment>(persisted.segment);
  snapshot.current_cycle = persisted.current_cycle;
  snapshot.remaining_seconds = persisted.remaining_seconds;
  snapshot.preset.focus_minutes = persisted.focus_minutes;
  snapshot.preset.break_minutes = persisted.break_minutes;
  snapshot.preset.cycles = persisted.cycles;
  snapshot.favourite.focus_minutes = persisted.favourite_focus;
  snapshot.favourite.break_minutes = persisted.favourite_break;
  snapshot.favourite.cycles = persisted.favourite_cycles;
  snapshot.completed_sessions = persisted.completed_sessions;
  return model.restore(snapshot);
}

bool PomodoroStore::save(const PomodoroSnapshot& snapshot) {
  PersistedState persisted{};
  persisted.magic = kMagic;
  persisted.version = kVersion;
  persisted.status = static_cast<std::uint8_t>(snapshot.status);
  persisted.segment = static_cast<std::uint8_t>(snapshot.segment);
  persisted.current_cycle = snapshot.current_cycle;
  persisted.cycles = snapshot.preset.cycles;
  persisted.focus_minutes = snapshot.preset.focus_minutes;
  persisted.break_minutes = snapshot.preset.break_minutes;
  persisted.favourite_focus = snapshot.favourite.focus_minutes;
  persisted.favourite_break = snapshot.favourite.break_minutes;
  persisted.favourite_cycles = snapshot.favourite.cycles;
  persisted.remaining_seconds = snapshot.remaining_seconds;
  persisted.completed_sessions = snapshot.completed_sessions;
  persisted.checksum = checksum(persisted);

  Preferences preferences;
  if (!preferences.begin(kNamespace, false)) {
    return false;
  }
  const size_t written =
      preferences.putBytes(kBlobKey, &persisted, sizeof(persisted));
  preferences.end();
  return written == sizeof(persisted);
}

}  // namespace kairos
