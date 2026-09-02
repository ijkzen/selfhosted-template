import type { LucideIcon } from "lucide-react";
import { Clock, FileText, Settings, Waypoints } from "lucide-react";

export interface PageConfig {
	path: string;
	/** 标题 i18n key（nav.pages.<page>.title），用于侧边栏与页面标题。 */
	titleKey: string;
	icon: LucideIcon;
}

/** 侧边栏分组：组标题 i18n key + 组内页面（顺序即展示顺序）。 */
export interface PageGroup {
	labelKey: string;
	pages: readonly PageConfig[];
}

export const HOME_PAGE: PageConfig = {
	path: "/",
	titleKey: "nav.pages.home.title",
	icon: Waypoints,
};

export const NOTES_PAGE: PageConfig = {
	path: "/notes",
	titleKey: "nav.pages.notes.title",
	icon: FileText,
};

export const CRON_JOBS_PAGE: PageConfig = {
	path: "/cron-jobs",
	titleKey: "nav.pages.cronJobs.title",
	icon: Clock,
};

export const SETTINGS_PAGE: PageConfig = {
	path: "/settings",
	titleKey: "nav.pages.settings.title",
	icon: Settings,
};

export const PAGES: readonly PageConfig[] = [HOME_PAGE, NOTES_PAGE, CRON_JOBS_PAGE, SETTINGS_PAGE];

/** 侧边栏导航分组。 */
export const NAV_GROUPS: readonly PageGroup[] = [
	{ labelKey: "nav.groups.main", pages: [HOME_PAGE, NOTES_PAGE] },
	{ labelKey: "nav.groups.admin", pages: [CRON_JOBS_PAGE, SETTINGS_PAGE] },
];
