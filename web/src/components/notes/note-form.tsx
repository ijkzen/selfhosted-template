import { Button } from "@/components/ui/button";
import {
	Form,
	FormControl,
	FormField,
	FormItem,
	FormLabel,
	FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import type { UpsertNoteRequest } from "@/hooks/use-notes";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { z } from "zod";

interface NoteFormProps {
	initial?: { title: string; content: string };
	onSubmit: (req: UpsertNoteRequest) => void;
	loading?: boolean;
}

export function NoteForm({ initial, onSubmit, loading }: NoteFormProps) {
	const { t } = useTranslation();
	const schema = z.object({
		title: z.string().min(1, t("notes.titleRequired")).max(200, t("notes.titleMax")),
		content: z.string().max(10_000, t("notes.contentMax")),
	});
	const form = useForm<UpsertNoteRequest>({
		resolver: zodResolver(schema),
		defaultValues: { title: initial?.title ?? "", content: initial?.content ?? "" },
	});

	return (
		<Form {...form}>
			<form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
				<FormField
					control={form.control}
					name="title"
					render={({ field }) => (
						<FormItem>
							<FormLabel required>{t("notes.title")}</FormLabel>
							<FormControl>
								<Input {...field} />
							</FormControl>
							<FormMessage />
						</FormItem>
					)}
				/>
				<FormField
					control={form.control}
					name="content"
					render={({ field }) => (
						<FormItem>
							<FormLabel>{t("notes.content")}</FormLabel>
							<FormControl>
								<Input {...field} />
							</FormControl>
							<FormMessage />
						</FormItem>
					)}
				/>
				<Button type="submit" disabled={loading} className="w-full">
					{initial ? t("common.save") : t("common.create")}
				</Button>
			</form>
		</Form>
	);
}
