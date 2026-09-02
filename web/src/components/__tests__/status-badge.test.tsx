import { StatusBadge } from "@/components/status-badge";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

describe("StatusBadge", () => {
	it("renders default enabled label", () => {
		render(<StatusBadge status="enabled" />);
		expect(screen.getByText("已启用")).toBeInTheDocument();
	});

	it("renders custom label", () => {
		render(<StatusBadge status="disabled" label="已禁用" />);
		expect(screen.getByText("已禁用")).toBeInTheDocument();
	});
});
