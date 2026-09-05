#pragma once

#include "PomodoroModel.h"

namespace kairos {

class PomodoroStore {
 public:
  bool begin();
  bool load(PomodoroModel& model);
  bool save(const PomodoroSnapshot& snapshot);
};

}  // namespace kairos
