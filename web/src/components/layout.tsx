import LocaleToggle from "@/components/locale-toggle";
import { PageHeaderSkeleton } from "@/components/page-header-skeleton";
import { SkipToMain } from "@/components/skip-to-main";
import { ThemeToggle } from "@/components/theme-toggle";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import {
	Sidebar,
	SidebarContent,
	SidebarFooter,
	SidebarGroup,
	SidebarGroupContent,
	SidebarGroupLabel,
	SidebarHeader,
	SidebarInset,
	SidebarMenu,
	SidebarMenuButton,
	SidebarMenuItem,
	SidebarProvider,
	SidebarTrigger,
} from "@/components/ui/sidebar";
import { Skeleton } from "@/components/ui/skeleton";
import { useLogout, useMe } from "@/hooks/use-auth";
import { fetchHealth } from "@/lib/api";
import { NAV_GROUPS } from "@/lib/pages";
import { cn } from "@/lib/utils";
import { type Query, useIsFetching, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronUp, LogOut, RefreshCw, Waypoints } from "lucide-react";
import { Suspense } from "react";
import { useTranslation } from "react-i18next";
import { Link, Outlet, useLocation, useNavigate } from "react-router-dom";

/** 页面刷新排除的布局级键：登录态清了会触发路由守卫全屏验证（失败即踢回登录页），版本号重取无意义。 */
const REFRESH_EXCLUDED_KEYS = ["auth", "health"];

/** 刷新范围与按钮忙碌态的同一判定：两个调用点必须一致，故共用此谓词。 */
function isRefreshableQuery(query: Query): boolean {
	return !REFRESH_EXCLUDED_KEYS.includes(String(query.queryKey[0]));
}

export default function AppLayout() {
	const { t } = useTranslation();
	const location = useLocation();
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const { data: me } = useMe();
	const logout = useLogout();
	const isRefreshing = useIsFetching({ predicate: isRefreshableQuery }) > 0;
	// 版本号动态读取（/api/healthz），发布新版无需改前端代码；取不到时只显示应用名。
	const { data: health } = useQuery({
		queryKey: ["health"],
		queryFn: fetchHealth,
		staleTime: 10 * 60 * 1000,
		retry: false,
	});

	const handleLogout = () => {
		logout.mutate(undefined, {
			onSettled: () => {
				queryClient.clear();
				navigate("/login", { replace: true });
			},
		});
	};

	// 嵌套页（如 /notes/:id）按路径前缀点亮所属一级导航。
	const isPageActive = (path: string) =>
		location.pathname === path || (path !== "/" && location.pathname.startsWith(`${path}/`));

	return (
		<SidebarProvider>
			<SkipToMain />
			<Sidebar variant="floating" className="sidebar-surface z-30">
				<SidebarHeader>
					<SidebarMenu>
						<SidebarMenuItem>
							<SidebarMenuButton size="lg" asChild>
								<Link to="/">
									<div className="flex aspect-square size-9 items-center justify-center rounded-md bg-primary text-primary-foreground">
										<Waypoints className="size-4" />
									</div>
									<div className="flex flex-col gap-0.5 leading-none">
										<span className="font-semibold">selfhosted-template</span>
										<span className="text-xs text-muted-foreground">{t("nav.appTitle")}</span>
									</div>
								</Link>
							</SidebarMenuButton>
						</SidebarMenuItem>
					</SidebarMenu>
				</SidebarHeader>
				<SidebarContent>
					{NAV_GROUPS.map((group) => (
						<SidebarGroup key={group.labelKey}>
							<SidebarGroupLabel>{t(group.labelKey)}</SidebarGroupLabel>
							<SidebarGroupContent>
								<SidebarMenu>
									{group.pages.map((page) => (
										<SidebarMenuItem key={page.path}>
											<SidebarMenuButton asChild isActive={isPageActive(page.path)}>
												<Link to={page.path}>
													<page.icon />
													<span>{t(page.titleKey)}</span>
												</Link>
											</SidebarMenuButton>
										</SidebarMenuItem>
									))}
								</SidebarMenu>
							</SidebarGroupContent>
						</SidebarGroup>
					))}
				</SidebarContent>
				<SidebarFooter>
					<SidebarMenu>
						<SidebarMenuItem>
							<DropdownMenu>
								<DropdownMenuTrigger asChild>
									<SidebarMenuButton size="lg" aria-label={t("nav.appTitle")}>
										<div className="flex aspect-square size-8 items-center justify-center rounded-full bg-foreground/10 text-sm font-semibold uppercase text-foreground">
											{(me?.username ?? "?").slice(0, 1)}
										</div>
										<div className="flex min-w-0 flex-col leading-none">
											<span className="truncate font-medium">{me?.username ?? "..."}</span>
											<span className="text-xs text-muted-foreground">{t("nav.loggedInAs")}</span>
										</div>
										<ChevronUp className="ml-auto size-4" />
									</SidebarMenuButton>
								</DropdownMenuTrigger>
								<DropdownMenuContent side="top" align="start" className="min-w-[180px]">
									<DropdownMenuLabel className="truncate">{me?.username}</DropdownMenuLabel>
									<DropdownMenuSeparator />
									<DropdownMenuItem variant="destructive" onClick={handleLogout}>
										<LogOut className="size-4" />
										{t("nav.logout")}
									</DropdownMenuItem>
								</DropdownMenuContent>
							</DropdownMenu>
						</SidebarMenuItem>
					</SidebarMenu>
					<div className="px-4 py-2 text-xs text-muted-foreground">
						<div>selfhosted-template{health?.version ? ` v${health.version}` : ""}</div>
					</div>
				</SidebarFooter>
			</Sidebar>
			{/* overflow-anchor:none 防止内容变化时滚动位置跳动；不能用 overflow-hidden，否则 sticky 顶栏失效 */}
			<SidebarInset className="[overflow-anchor:none]">
				{/* 吸顶样式由 CSS scroll-state 查询驱动，见 sticky-header.css 的 .app-header */}
				<header className="app-header sticky top-0 z-10 shrink-0">
					<div className="app-header-inner flex h-14 items-center gap-4 px-6">
						<SidebarTrigger className="-ml-2" aria-label={t("nav.appTitle")} />
						<Separator orientation="vertical" className="h-6" />
						<div className="ml-auto flex shrink-0 items-center gap-2">
							{/* 页面刷新：清空除布局级键外的查询缓存并立即重取当前页面。
							    用 resetQueries（先清数据再重取 active，忽略 staleTime）而非
							    removeQueries（active 不自动重取）或 invalidateQueries（不删数据）。 */}
							<Button
								variant="outline"
								size="icon"
								title={t("common.refresh")}
								aria-label={t("common.refresh")}
								disabled={isRefreshing}
								onClick={() => queryClient.resetQueries({ predicate: isRefreshableQuery })}
							>
								<RefreshCw className={cn("size-4", isRefreshing && "animate-spin")} />
							</Button>
							<LocaleToggle />
							<ThemeToggle />
						</div>
					</div>
				</header>
				<div id="content" className="mx-auto flex w-full max-w-7xl flex-1 flex-col gap-6 p-6">
					<Suspense
						fallback={
							<div className="space-y-6" aria-busy="true" aria-live="polite">
								<PageHeaderSkeleton />
								<Skeleton className="h-[260px] w-full" />
								<Skeleton className="h-[260px] w-full" />
							</div>
						}
					>
						<div key={location.pathname} className="page-enter">
							<Outlet />
						</div>
					</Suspense>
				</div>
			</SidebarInset>
		</SidebarProvider>
	);
}
