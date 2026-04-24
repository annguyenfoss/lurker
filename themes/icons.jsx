// Icons — line-based, 1.75 stroke, 16 viewBox
const svgProps = { viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: 1.5, strokeLinecap: "round", strokeLinejoin: "round" };

const Icon = {
  Lock: (p) => (<svg {...svgProps} {...p}><rect x="3.5" y="7" width="9" height="6.5" rx="1.2"/><path d="M5.5 7V4.8a2.5 2.5 0 0 1 5 0V7"/></svg>),
  LockOpen: (p) => (<svg {...svgProps} {...p}><rect x="3.5" y="7" width="9" height="6.5" rx="1.2"/><path d="M5.5 7V4.8a2.5 2.5 0 0 1 4.8-.7"/></svg>),
  Shield: (p) => (<svg {...svgProps} {...p}><path d="M8 2l5 2v4.5c0 3-2.2 5.5-5 6.5-2.8-1-5-3.5-5-6.5V4l5-2z"/></svg>),
  Plus: (p) => (<svg {...svgProps} {...p}><path d="M8 3.5v9M3.5 8h9"/></svg>),
  Eject: (p) => (<svg {...svgProps} {...p}><path d="M3.5 13h9"/><path d="M8 3l4.5 7h-9L8 3z"/></svg>),
  Folder: (p) => (<svg {...svgProps} {...p}><path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.4a1.5 1.5 0 0 1 1.2.6l.6.8a1.5 1.5 0 0 0 1.2.6H12.5A1.5 1.5 0 0 1 14 6.5v5A1.5 1.5 0 0 1 12.5 13h-9A1.5 1.5 0 0 1 2 11.5v-7z"/></svg>),
  File: (p) => (<svg {...svgProps} {...p}><path d="M4 2h5l3 3v9H4V2z"/><path d="M9 2v3h3"/></svg>),
  Key: (p) => (<svg {...svgProps} {...p}><circle cx="5.5" cy="9.5" r="2.5"/><path d="M7.5 8l5-5M10.5 5l1.5 1.5M12.5 3l1.5 1.5"/></svg>),
  Eye: (p) => (<svg {...svgProps} {...p}><path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8s-2.5 4.5-6.5 4.5S1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/></svg>),
  EyeOff: (p) => (<svg {...svgProps} {...p}><path d="M3 3l10 10M6.5 5a7.5 7.5 0 0 1 1.5-.2c4 0 6.5 3.2 6.5 3.2a13 13 0 0 1-2 2.3M10 10.5a2 2 0 0 1-3-2.5M2 6.5A12 12 0 0 0 1.5 8s2.5 4.5 6.5 4.5c.9 0 1.7-.2 2.4-.5"/></svg>),
  Dice: (p) => (<svg {...svgProps} {...p}><rect x="2.5" y="2.5" width="11" height="11" rx="2"/><circle cx="5.5" cy="5.5" r=".6" fill="currentColor" stroke="none"/><circle cx="10.5" cy="10.5" r=".6" fill="currentColor" stroke="none"/><circle cx="8" cy="8" r=".6" fill="currentColor" stroke="none"/></svg>),
  Chevron: (p) => (<svg {...svgProps} {...p}><path d="M6 4l4 4-4 4"/></svg>),
  Check: (p) => (<svg {...svgProps} {...p}><path d="M3 8l3.5 3.5L13 5"/></svg>),
  X: (p) => (<svg {...svgProps} {...p}><path d="M4 4l8 8M12 4l-8 8"/></svg>),
  Info: (p) => (<svg {...svgProps} {...p}><circle cx="8" cy="8" r="6"/><path d="M8 7.5v3.5M8 5h0"/></svg>),
  Refresh: (p) => (<svg {...svgProps} {...p}><path d="M14 3v3h-3M2 13v-3h3"/><path d="M13.5 6.5A6 6 0 0 0 3.5 5M2.5 9.5A6 6 0 0 0 12.5 11"/></svg>),
  Terminal: (p) => (<svg {...svgProps} {...p}><rect x="1.5" y="3" width="13" height="10" rx="1.5"/><path d="M4 6.5L6.5 8 4 9.5M7.5 10h3"/></svg>),
  Drive: (p) => (<svg {...svgProps} {...p}><rect x="2" y="4" width="12" height="5" rx="1"/><rect x="2" y="9" width="12" height="4" rx="1"/><circle cx="4.5" cy="6.5" r=".6" fill="currentColor" stroke="none"/><circle cx="4.5" cy="11" r=".6" fill="currentColor" stroke="none"/></svg>),
  Min: (p) => (<svg {...svgProps} {...p} strokeWidth="1"><path d="M4 8h8"/></svg>),
  Max: (p) => (<svg {...svgProps} {...p} strokeWidth="1"><rect x="4" y="4" width="8" height="8"/></svg>),
  CloseW: (p) => (<svg {...svgProps} {...p} strokeWidth="1"><path d="M4 4l8 8M12 4l-8 8"/></svg>),
  Search: (p) => (<svg {...svgProps} {...p}><circle cx="7" cy="7" r="4"/><path d="M10 10l3 3"/></svg>),
  Sparkle: (p) => (<svg {...svgProps} {...p}><path d="M8 2l1 3 3 1-3 1-1 3-1-3-3-1 3-1 1-3zM12.5 9l.5 1.5 1.5.5-1.5.5-.5 1.5-.5-1.5-1.5-.5 1.5-.5.5-1.5z"/></svg>),
  Copy: (p) => (<svg {...svgProps} {...p}><rect x="5" y="5" width="8" height="8" rx="1"/><path d="M3 11V3.5A.5.5 0 0 1 3.5 3H11"/></svg>),
  Sun: (p) => (<svg {...svgProps} {...p}><circle cx="8" cy="8" r="2.5"/><path d="M8 1.5v1.5M8 13v1.5M1.5 8h1.5M13 8h1.5M3.3 3.3l1 1M11.7 11.7l1 1M12.7 3.3l-1 1M4.3 11.7l-1 1"/></svg>),
  Moon: (p) => (<svg {...svgProps} {...p}><path d="M13 9.5A5.5 5.5 0 0 1 6.5 3a5.5 5.5 0 1 0 6.5 6.5z"/></svg>),
};

window.Icon = Icon;
