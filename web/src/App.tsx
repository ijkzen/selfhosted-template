import AppLayout from "@/components/layout";
import { RequireAuth } from "@/components/require-auth";
import { ScrollToTop } from "@/components/scroll-to-top";
import { Toaster } from "@/components/ui/sonner";
import { useInitLocale } from "@/hooks/use-locale";
import { useInitTheme } from "@/hooks/use-theme";
import { Suspense, lazy } from "react";
import { Route, Routes } from "react-router-dom";

const LoginPage = lazy(() => import("./pages/login"));
const NotesPage = lazy(() => import("./pages/notes"));
const CronJobsPage = lazy(() => import("./pages/cron-jobs"));
const SettingsPage = lazy(() => import("./pages/settings"));
const NotFoundPage = lazy(() => import("./pages/not-found"));

function App() {
	useInitTheme();
	useInitLocale();

	return (
		<>
			<ScrollToTop />
			{/* /login 在 AppLayout 之外，若只靠 layout 内的 Suspense，直达或硬刷新
			    登录页时 chunk 未就绪会白屏挂起——这里兜住整棵路由树。 */}
			<Suspense
				fallback={
					<div className="flex min-h-screen items-center justify-center text-sm text-muted-foreground">
						…
					</div>
				}
			>
				<Routes>
					<Route
						element={
							<RequireAuth>
								<AppLayout />
							</RequireAuth>
						}
					>
						<Route
							index
							element={
								<div className="p-6">
									<h1 className="text-2xl font-bold">selfhosted-template</h1>
									<p className="text-muted-foreground mt-2">Welcome to your new project.</p>
								</div>
							}
						/>
						<Route path="/notes" element={<NotesPage />} />
						<Route path="/cron-jobs" element={<CronJobsPage />} />
						<Route path="/settings" element={<SettingsPage />} />
						<Route path="*" element={<NotFoundPage />} />
					</Route>
					<Route path="/login" element={<LoginPage />} />
				</Routes>
			</Suspense>
			<Toaster />
		</>
	);
}

export default App;
