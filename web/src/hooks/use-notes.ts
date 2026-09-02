import { type ApiResponse, api, unwrap } from "@/lib/api";

export interface Note {
	id: number;
	title: string;
	content: string;
	createdAt: string;
	updatedAt: string;
}

export interface UpsertNoteRequest {
	title: string;
	content: string;
}

export async function listNotes(): Promise<Note[]> {
	return unwrap(await api.get("notes").json<ApiResponse<Note[]>>());
}

export async function createNote(req: UpsertNoteRequest): Promise<Note> {
	return unwrap(await api.post("notes", { json: req }).json<ApiResponse<Note>>());
}

export async function updateNote(id: number, req: UpsertNoteRequest): Promise<Note> {
	return unwrap(await api.put(`notes/${id}`, { json: req }).json<ApiResponse<Note>>());
}

export async function deleteNote(id: number): Promise<void> {
	await unwrap(await api.delete(`notes/${id}`).json<ApiResponse<void>>());
}
