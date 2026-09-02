import { ConfirmDialog } from "@/components/confirm-dialog";
import { EmptyState } from "@/components/empty-state";
import { ErrorState } from "@/components/error-state";
import { NoteForm } from "@/components/notes/note-form";
import { PageHeader } from "@/components/page-header";
import { TableSkeleton } from "@/components/table-skeleton";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import {
	type Note,
	type UpsertNoteRequest,
	createNote,
	deleteNote,
	listNotes,
	updateNote,
} from "@/hooks/use-notes";
import { useToastActions } from "@/hooks/use-toast";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Plus } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

export default function NotesPage() {
	const { t, i18n } = useTranslation();
	const queryClient = useQueryClient();
	const { toastSuccess, toastError } = useToastActions();
	const [editNote, setEditNote] = useState<Note | null>(null);
	const [deleteNoteId, setDeleteNoteId] = useState<number | null>(null);
	const [showCreate, setShowCreate] = useState(false);

	const { data, isLoading, isError } = useQuery({
		queryKey: ["notes"],
		queryFn: listNotes,
	});

	const createMutation = useMutation({
		mutationFn: createNote,
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["notes"] });
			setShowCreate(false);
			toastSuccess(t("common.createSuccess"));
		},
		onError: (e: Error) => toastError(t("common.createFailed"), e),
	});

	const updateMutation = useMutation({
		mutationFn: ({ id, req }: { id: number; req: UpsertNoteRequest }) => updateNote(id, req),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["notes"] });
			setEditNote(null);
			toastSuccess(t("common.updateSuccess"));
		},
		onError: (e: Error) => toastError(t("common.updateFailed"), e),
	});

	const deleteMutation = useMutation({
		mutationFn: (id: number) => deleteNote(id),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["notes"] });
			setDeleteNoteId(null);
			toastSuccess(t("common.deleteSuccess"));
		},
		onError: (e: Error) => toastError(t("common.deleteFailed"), e),
	});

	if (isLoading) return <TableSkeleton columns={3} />;
	if (isError)
		return <ErrorState onRetry={() => queryClient.invalidateQueries({ queryKey: ["notes"] })} />;

	const notes = data ?? [];

	return (
		<div className="space-y-6">
			<PageHeader icon={FileText} title={t("notes.title")}>
				<Button onClick={() => setShowCreate(true)}>
					<Plus className="size-4" />
					{t("notes.create")}
				</Button>
			</PageHeader>

			{notes.length === 0 ? (
				<EmptyState
					icon={FileText}
					title={t("notes.emptyTitle")}
					description={t("notes.emptyDesc")}
					action={
						<Button onClick={() => setShowCreate(true)}>
							<Plus className="size-4" />
							{t("notes.create")}
						</Button>
					}
				/>
			) : (
				<Card className="overflow-x-auto">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>{t("notes.title")}</TableHead>
								<TableHead className="hidden sm:table-cell">{t("notes.updatedAt")}</TableHead>
								<TableHead className="w-24">{t("common.actions")}</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{notes.map((note) => (
								<TableRow key={note.id}>
									<TableCell className="font-medium">{note.title}</TableCell>
									<TableCell className="hidden text-muted-foreground sm:table-cell">
										{new Date(note.updatedAt).toLocaleString(i18n.language)}
									</TableCell>
									<TableCell>
										<div className="flex gap-1">
											<Button variant="outline" size="sm" onClick={() => setEditNote(note)}>
												{t("common.edit")}
											</Button>
											<Button
												variant="destructive"
												size="sm"
												onClick={() => setDeleteNoteId(note.id)}
											>
												{t("common.delete")}
											</Button>
										</div>
									</TableCell>
								</TableRow>
							))}
						</TableBody>
					</Table>
				</Card>
			)}

			{showCreate && (
				<Dialog open onOpenChange={(open) => !open && setShowCreate(false)}>
					<DialogContent>
						<DialogHeader>
							<DialogTitle>{t("notes.create")}</DialogTitle>
						</DialogHeader>
						<NoteForm
							onSubmit={(req) => createMutation.mutate(req)}
							loading={createMutation.isPending}
						/>
					</DialogContent>
				</Dialog>
			)}

			{editNote && (
				<Dialog open onOpenChange={(open) => !open && setEditNote(null)}>
					<DialogContent>
						<DialogHeader>
							<DialogTitle>{t("notes.edit")}</DialogTitle>
						</DialogHeader>
						<NoteForm
							initial={editNote}
							onSubmit={(req) => updateMutation.mutate({ id: editNote.id, req })}
							loading={updateMutation.isPending}
						/>
					</DialogContent>
				</Dialog>
			)}

			<ConfirmDialog
				open={deleteNoteId !== null}
				onOpenChange={(open) => {
					if (!open) setDeleteNoteId(null);
				}}
				title={t("notes.deleteConfirm")}
				destructive
				isLoading={deleteMutation.isPending}
				handleConfirm={() => {
					if (deleteNoteId !== null) deleteMutation.mutate(deleteNoteId);
				}}
			/>
		</div>
	);
}
