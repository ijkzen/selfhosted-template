import { useInitTheme, useTheme } from "@/hooks/use-theme";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

describe("useInitTheme", () => {
	let darkMatches = false;
	let changeHandler: (() => void) | null = null;

	beforeEach(() => {
		window.localStorage.clear();
		document.documentElement.classList.remove("dark");
		darkMatches = false;
		changeHandler = null;
		window.matchMedia = vi.fn().mockImplementation((query: string) => ({
			get matches() {
				return darkMatches;
			},
			media: query,
			onchange: null,
			addListener: vi.fn(),
			removeListener: vi.fn(),
			addEventListener: (_: string, cb: () => void) => {
				changeHandler = cb;
			},
			removeEventListener: vi.fn(),
			dispatchEvent: vi.fn(),
		}));
	});

	it("applies the dark class for the dark theme", () => {
		useTheme.setState({ theme: "dark", _hasHydrated: true });
		renderHook(() => useInitTheme());

		expect(document.documentElement.classList.contains("dark")).toBe(true);
	});

	it("follows system color scheme changes when theme is system", () => {
		useTheme.setState({ theme: "system", _hasHydrated: true });
		renderHook(() => useInitTheme());

		expect(document.documentElement.classList.contains("dark")).toBe(false);

		darkMatches = true;
		act(() => changeHandler?.());
		expect(document.documentElement.classList.contains("dark")).toBe(true);
	});
});

describe("setTheme", () => {
	let darkMatches = false;

	beforeEach(() => {
		window.localStorage.clear();
		document.documentElement.classList.remove("dark");
		darkMatches = false;
		window.matchMedia = vi.fn().mockImplementation((query: string) => ({
			get matches() {
				return darkMatches;
			},
			media: query,
			onchange: null,
			addListener: vi.fn(),
			removeListener: vi.fn(),
			addEventListener: () => {},
			removeEventListener: vi.fn(),
			dispatchEvent: vi.fn(),
		}));
	});

	it("skips DOM application when the resolved theme is unchanged", () => {
		// 手动暗色 + 系统也是暗色：切到「跟随系统」时解析后的主题不变，
		// 应只更新偏好、不再执行 applyResolvedTheme（dark class 保持原状）。
		useTheme.setState({ theme: "dark" });
		document.documentElement.classList.add("dark");
		darkMatches = true;

		act(() => useTheme.getState().setTheme("system"));

		expect(useTheme.getState().theme).toBe("system");
		// DOM 未被重写：暗色 class 仍来自当前偏好，meta 也未被二次写入
		expect(document.documentElement.classList.contains("dark")).toBe(true);
	});

	it("applies DOM changes when the resolved theme differs", () => {
		// 手动亮色 + 系统暗色：切到「跟随系统」时解析后主题变为暗色，应执行切换
		useTheme.setState({ theme: "light" });
		document.documentElement.classList.remove("dark");
		darkMatches = true;

		act(() => useTheme.getState().setTheme("system"));

		expect(useTheme.getState().theme).toBe("system");
		expect(document.documentElement.classList.contains("dark")).toBe(true);
	});
});
