import React from "react";

type Brain = "Everyday" | "PhD" | "Datter" | "Border Fiber" | "Restricted" | "File";
const colors: Record<Brain,string> = { Everyday:"#4C9FFF", PhD:"#9E7BFF", Datter:"#49D7D2", "Border Fiber":"#FFB85C", Restricted:"#FF6B6B", File:"#A5B4CC" };

export function CitationChip({ brain, detail }: { brain: Brain; detail?: string }) {
  const c = colors[brain];
  return <span className="kairos-citation-chip" style={{ "--chip-color": c } as React.CSSProperties}>
    <span className="kairos-citation-chip__dot" aria-hidden="true"/>
    <span>{brain}</span>{detail && <span className="kairos-citation-chip__detail">{detail}</span>}
  </span>;
}
