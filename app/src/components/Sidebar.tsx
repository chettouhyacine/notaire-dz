import type { JSX } from 'react';
import type { MessageKey } from '../i18n';

export interface NavItem {
  id: ScreenId;
  labelKey: MessageKey;
}

/** Exactly eight entries (Section 14). The order is data, not markup. */
export type ScreenId =
  | 'home'
  | 'cases'
  | 'clients'
  | 'acts'
  | 'accounting'
  | 'compliance'
  | 'calendar'
  | 'settings';

export interface SidebarProps {
  items: readonly NavItem[];
  current: ScreenId;
  onSelect: (id: ScreenId) => void;
  renderIcon: (id: ScreenId) => JSX.Element;
  renderLabel: (key: MessageKey) => string;
  ariaLabel: string;
}

export function Sidebar({
  items,
  current,
  onSelect,
  renderIcon,
  renderLabel,
  ariaLabel,
}: SidebarProps): JSX.Element {
  return (
    <nav className="sidebar" aria-label={ariaLabel}>
      <ul>
        {items.map((item) => (
          <li key={item.id}>
            <button
              type="button"
              className="sidebar-item"
              aria-current={item.id === current ? 'page' : undefined}
              onClick={() => onSelect(item.id)}
            >
              <span className="sidebar-icon">{renderIcon(item.id)}</span>
              <span className="sidebar-label">{renderLabel(item.labelKey)}</span>
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
