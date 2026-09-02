import AppLayout from "@/components/layout";
import { RequireAuth } from "@/components/require-auth";
import { ScrollToTop } from "@/components/scroll-to-top";
import { Toaster } from "@/components/ui/sonner";
import { useInitLocale } from "@/hooks/use-locale";
import { useInitTheme } from "@/hooks/use-theme";
import { lazy } from "react";
import { Route, Routes } from "react-router-dom";

const LoginPage = lazy(() => import("./pages/login"));
const NotesPage = lazy(() => import("./pages/notes"));
const NotFoundPage = lazy(() => import("./pages/not-found"));

function App() {
	useInitTheme();
	useInitLocale();

	return (
		<>
			<ScrollToTop />
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
					<Route path="*" element={<NotFoundPage />} />
				</Route>
				<Route path="/login" element={<LoginPage />} />
			</Routes>
			<Toaster />
		</>
	);
}

export default App;
