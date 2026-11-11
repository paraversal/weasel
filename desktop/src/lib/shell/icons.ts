import Box from '@lucide/svelte/icons/box';
import CircleHelp from '@lucide/svelte/icons/circle-help';
import Code from '@lucide/svelte/icons/code';
import HistoryIcon from '@lucide/svelte/icons/history';
import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard';
import Mail from '@lucide/svelte/icons/mail';
import SettingsIcon from '@lucide/svelte/icons/settings';
import Upload from '@lucide/svelte/icons/upload';
import Users from '@lucide/svelte/icons/users';

/**
 * The only icon set manifests may reference. Registering an icon here makes
 * `icon: "usr"` a compile error instead of a missing glyph at runtime.
 */
export const icons = {
	dashboard: LayoutDashboard,
	users: Users,
	mail: Mail,
	history: HistoryIcon,
	upload: Upload,
	settings: SettingsIcon,
	help: CircleHelp,
	code: Code,
	box: Box
};

export type IconName = keyof typeof icons;
