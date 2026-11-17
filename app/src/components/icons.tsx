import type { JSX, SVGProps } from 'react';

/**
 * Icons are drawn inline: no icon font, no remote SVG, no network request.
 * They are decorative next to a text label, so they never carry meaning alone.
 */
type IconProps = { size?: number };

const base = (size: number): SVGProps<SVGSVGElement> => ({
  width: size,
  height: size,
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.8,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
  'aria-hidden': true,
  focusable: false,
});

export const IconHome = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <path d="M3 10.5 12 3l9 7.5" />
    <path d="M5 9.5V21h14V9.5" />
    <path d="M10 21v-6h4v6" />
  </svg>
);

export const IconCases = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <path d="M3 7h6l2 2h10v11H3z" />
    <path d="M3 7V4h6l2 2" />
  </svg>
);

export const IconClients = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <circle cx="9" cy="8" r="3.2" />
    <path d="M3.5 20c0-3 2.5-5.2 5.5-5.2s5.5 2.2 5.5 5.2" />
    <circle cx="17" cy="9" r="2.4" />
    <path d="M15.5 14.6c2.6.3 4.5 2.3 4.5 5.4" />
  </svg>
);

export const IconActs = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v4h4" />
    <path d="M9 12h6M9 16h6" />
  </svg>
);

export const IconAccounting = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <path d="M12 3v18" />
    <path d="M5 7h14M5 7l-3 6h6zM19 7l3 6h-6z" />
    <path d="M8 21h8" />
  </svg>
);

export const IconCompliance = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <path d="M12 3 5 6v6c0 4.2 2.9 7.6 7 9 4.1-1.4 7-4.8 7-9V6z" />
    <path d="m9 12 2.2 2.2L15.5 10" />
  </svg>
);

export const IconCalendar = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <rect x="3.5" y="5" width="17" height="16" rx="2" />
    <path d="M3.5 10h17M8 3v4M16 3v4" />
  </svg>
);

export const IconSettings = ({ size = 20 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <circle cx="12" cy="12" r="3.2" />
    <path d="M12 2.5v3M12 18.5v3M4.2 7l2.6 1.5M17.2 15.5l2.6 1.5M4.2 17l2.6-1.5M17.2 8.5l2.6-1.5" />
  </svg>
);

export const IconSearch = ({ size = 18 }: IconProps): JSX.Element => (
  <svg {...base(size)}>
    <circle cx="10.5" cy="10.5" r="6.5" />
    <path d="m15.5 15.5 5 5" />
  </svg>
);
