import type { SpecialistResponseIdentityV1 } from "./native/specialistContracts";

export type StoredChatMessageV1 = {
  id: string;
  role: "user" | "assistant";
  content: string;
  createdAt: string;
  sourceIds?: string[];
  providerLabel?: string;
  routeBrainIds?: string[];
  specialistIdentity?: SpecialistResponseIdentityV1;
};

export function hydrateStoredChatMessage(
  message: StoredChatMessageV1,
  brainName: (brainId: string) => string,
) {
  return {
    id: message.id,
    role: message.role,
    body: message.content,
    createdAt: message.createdAt,
    provider: message.providerLabel,
    routes: (message.routeBrainIds ?? []).map((id) => ({ id, name: brainName(id) })),
    citations: (message.sourceIds ?? []).map((id) => ({
      id,
      brainId: id.split(":")[0] ?? "",
      relativePath: id.split(":").slice(1).join(":"),
    })),
    specialistIdentity: message.specialistIdentity,
  };
}
