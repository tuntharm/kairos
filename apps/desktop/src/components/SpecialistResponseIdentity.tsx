import type { SpecialistResponseIdentityV1 } from "../native/specialistContracts";

type SpecialistResponseIdentityProps = {
  identity: SpecialistResponseIdentityV1;
};

export function SpecialistResponseIdentity({ identity }: SpecialistResponseIdentityProps) {
  return (
    <details className="specialist-response-identity">
      <summary>
        <span>Specialist evidence</span>
        <strong>{identity.specialistName}</strong>
      </summary>
      <dl>
        <div><dt>Specialist</dt><dd><code>{identity.specialistId}</code></dd></div>
        <div><dt>Release</dt><dd><code>{identity.releaseId}</code></dd></div>
        <div><dt>Evidence boundary</dt><dd><code>{identity.evidenceBoundarySha256}</code></dd></div>
        <div><dt>Evaluation</dt><dd><code>{identity.evaluationSha256}</code></dd></div>
        <div><dt>Release record</dt><dd><code>{identity.releaseSha256}</code></dd></div>
      </dl>
    </details>
  );
}
