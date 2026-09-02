import NotesPage from "@/pages/notes";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const listNotesMock = vi.fn();
const createNoteMock = vi.fn();
const updateNoteMock = vi.fn();
const deleteNoteMock = vi.fn();

vi.mock("@/hooks/use-notes", () => ({
	listNotes: () => listNotesMock(),
	createNote: (req: unknown) => createNoteMock(req),
	updateNote: (id: number, req: unknown) => updateNoteMock(id, req),
	deleteNote: (id: number) => deleteNoteMock(id),
}));

vi.mock("@/hooks/use-toast", () => ({
	useToastActions: () => ({ toastSuccess: vi.fn(), toastError: vi.fn() }),
}));

function renderPage() {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return render(
		<QueryClientProvider client={queryClient}>
			<NotesPage />
		</QueryClientProvider>,
	);
}

describe("NotesPage", () => {
	it("renders notes list", async () => {
		listNotesMock.mockResolvedValue([
			{ id: 1, title: "First", content: "hello", createdAt: "", updatedAt: "" },
		]);
		renderPage();
		expect(await screen.findByText("First")).toBeInTheDocument();
	});

	it("shows empty state when no notes", async () => {
		listNotesMock.mockResolvedValue([]);
		renderPage();
		expect(await screen.findByText("还没有笔记")).toBeInTheDocument();
	});
});
