import React from "react";

type KairosMarkProps = { size?: number; className?: string; title?: string };

export function KairosMark({ size = 32, className, title = "Kairos" }: KairosMarkProps) {
  const id = React.useId().replace(/:/g, "");
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 1024 1024" role="img" aria-label={title}>
      <defs>
        <linearGradient id={`${id}-ring`} x1="0" y1="0" x2="1" y2="1">
          <stop stopColor="#3C8DFF"/><stop offset=".48" stopColor="#B8D8FF"/><stop offset=".78" stopColor="#F4F7FF"/><stop offset="1" stopColor="#FFC766"/>
        </linearGradient>
        <linearGradient id={`${id}-k`} x1="0" y1="0" x2="1" y2="1">
          <stop stopColor="#76E4FF"/><stop offset=".42" stopColor="#A9CCFF"/><stop offset=".8" stopColor="#F4F7FF"/>
        </linearGradient>
      </defs>
      <path d="M737 244A350 350 0 1 0 737 780" fill="none" stroke={`url(#${id}-ring)`} strokeWidth="56" strokeLinecap="round"/>
      <g fill="none" stroke={`url(#${id}-k)`} strokeWidth="50" strokeLinecap="round" strokeLinejoin="round">
        <path d="M404 332v98q0 25 44 63"/><path d="M404 692v-98q0-25 44-63"/><path d="M495 493 670 338"/><path d="M495 531 670 686"/>
      </g>
      {[ [404,322],[404,702],[680,329],[680,695] ].map(([cx,cy],i)=><circle key={i} cx={cx} cy={cy} r="34" fill={`url(#${id}-k)`}/>) }
      <circle cx="472" cy="512" r="48" fill="#07142B" stroke={`url(#${id}-k)`} strokeWidth="22"/>
      <circle cx="795" cy="512" r="34" fill="#FFC766"/>
    </svg>
  );
}
