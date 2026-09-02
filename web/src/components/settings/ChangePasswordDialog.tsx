import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import {
	Form,
	FormControl,
	FormField,
	FormItem,
	FormLabel,
	FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { useChangePassword } from "@/hooks/use-auth";
import { useToastActions } from "@/hooks/use-toast";
import { zodResolver } from "@hookform/resolvers/zod";
import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { z } from "zod";

interface ChangePasswordDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

export function ChangePasswordDialog({ open, onOpenChange }: ChangePasswordDialogProps) {
	const { t } = useTranslation();
	const { toastSuccess, toastError } = useToastActions();
	const changePassword = useChangePassword();

	const changePasswordSchema = z
		.object({
			oldPassword: z.string().min(1, t("settings.oldPasswordRequired")),
			newPassword: z
				.string()
				.min(6, t("settings.newPasswordMin"))
				.max(128, t("settings.newPasswordMax")),
			confirmPassword: z.string(),
		})
		.refine((values) => values.newPassword === values.confirmPassword, {
			message: t("settings.passwordMismatch"),
			path: ["confirmPassword"],
		});

	const form = useForm({
		resolver: zodResolver(changePasswordSchema),
		defaultValues: { oldPassword: "", newPassword: "", confirmPassword: "" },
	});

	useEffect(() => {
		if (!open) {
			form.reset();
		}
	}, [open, form]);

	const onSubmit = (values: { oldPassword: string; newPassword: string }) => {
		changePassword.mutate(
			{ oldPassword: values.oldPassword, newPassword: values.newPassword },
			{
				onSuccess: () => {
					onOpenChange(false);
					toastSuccess(t("settings.changeSuccess"));
				},
				onError: (error) => {
					toastError(t("settings.changeFailed"), error);
				},
			},
		);
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-[440px]">
				<DialogHeader className="space-y-3">
					<DialogTitle>{t("settings.changePassword")}</DialogTitle>
					<DialogDescription>{t("settings.changePasswordDesc")}</DialogDescription>
				</DialogHeader>
				<Form {...form}>
					<form onSubmit={form.handleSubmit(onSubmit)}>
						<div className="grid gap-4 py-2">
							<FormField
								control={form.control}
								name="oldPassword"
								render={({ field }) => (
									<FormItem>
										<FormLabel>{t("settings.oldPassword")}</FormLabel>
										<FormControl>
											<Input type="password" autoComplete="current-password" {...field} />
										</FormControl>
										<FormMessage />
									</FormItem>
								)}
							/>
							<FormField
								control={form.control}
								name="newPassword"
								render={({ field }) => (
									<FormItem>
										<FormLabel>{t("settings.newPassword")}</FormLabel>
										<FormControl>
											<Input type="password" autoComplete="new-password" {...field} />
										</FormControl>
										<FormMessage />
									</FormItem>
								)}
							/>
							<FormField
								control={form.control}
								name="confirmPassword"
								render={({ field }) => (
									<FormItem>
										<FormLabel>{t("settings.confirmNewPassword")}</FormLabel>
										<FormControl>
											<Input type="password" autoComplete="new-password" {...field} />
										</FormControl>
										<FormMessage />
									</FormItem>
								)}
							/>
						</div>
						<DialogFooter className="gap-2">
							<Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
								{t("common.cancel")}
							</Button>
							<Button type="submit" disabled={changePassword.isPending}>
								{t("settings.confirmChange")}
							</Button>
						</DialogFooter>
					</form>
				</Form>
			</DialogContent>
		</Dialog>
	);
}
