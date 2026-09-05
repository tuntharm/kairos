import type { SpecialistSummaryV1 } from "./native/specialistContracts";

export type ChatTarget = "manager" | `specialist:${string}`;

export type ChatDelivery =
  | { kind: "manager_local"; badge: "LOCAL"; specialist?: undefined }
  | { kind: "manager_external"; badge: "PREVIEW FIRST"; specialist?: undefined }
  | { kind: "specialist_local"; badge: "LOCAL SPECIALIST"; specialist: SpecialistSummaryV1 }
  | { kind: "specialist_unavailable"; badge: "SPECIALIST PAUSED"; specialist?: undefined };

export function resolveChatDelivery(
  target: ChatTarget,
  specialists: SpecialistSummaryV1[],
  managerIsExternal: boolean,
): ChatDelivery {
  if (target === "manager") {
    return managerIsExternal
      ? { kind: "manager_external", badge: "PREVIEW FIRST" }
      : { kind: "manager_local", badge: "LOCAL" };
  }

  const specialistId = target.slice("specialist:".length);
  const specialist = specialists.find((candidate) => (
    candidate.specialistId === specialistId && Boolean(candidate.activeReleaseId)
  ));
  return specialist
    ? { kind: "specialist_local", badge: "LOCAL SPECIALIST", specialist }
    : { kind: "specialist_unavailable", badge: "SPECIALIST PAUSED" };
}
