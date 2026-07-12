import React from "react";
import "./kairos.css";

export function KairosLoader({ size = 48, label = "Kairos is routing" }: { size?: number; label?: string }) {
  return (
    <svg className="kairos-loader" width={size} height={size} viewBox="0 0 64 64" role="status" aria-label={label}>
      <path className="kairos-loader__ring" d="M46 13a23 23 0 1 0 0 38" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="round"/>
      <g fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round">
        <path d="M25 19v9l7 4 9-8"/><path d="m32 32 9 8"/><path d="M25 36v9"/>
      </g>
      <circle className="kairos-loader__core" cx="32" cy="32" r="4" fill="#76E4FF"/>
      <circle className="kairos-loader__moment" cx="50" cy="32" r="4" fill="#FFC766"/>
    </svg>
  );
}
