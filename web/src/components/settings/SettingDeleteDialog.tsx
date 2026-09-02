import { ConfirmDialog } from "@/components/confirm-dialog";
import { type Setting, useDeleteSetting } from "@/hooks/use-settings";
import { useToastActions } from "@/hooks/use-toast";
import { useTranslation } from "react-i18next";

interface SettingDeleteDialogProps {
	setting: Setting | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

/** 删除设置二次确认弹窗：展示将被删除的 key，确认后调用删除接口。 */
export function SettingDeleteDialog({ setting, open, onOpenChange }: SettingDeleteDialogProps) {
	const { t } = useTranslation();
	const { toastSuccess, toastError } = useToastActions();
	const deleteSetting = useDeleteSetting();

	const handleConfirm = () => {
		if (!setting) return;
		deleteSetting.mutate(setting.key, {
			onSuccess: () => {
				onOpenChange(false);
				toastSuccess(t("common.deleteSuccess"));
			},
			onError: (error) => {
				toastError(t("common.deleteFailed"), error);
			},
		});
	};

	return (
		<ConfirmDialog
			open={open}
			onOpenChange={onOpenChange}
			title={t("settings.deleteSetting")}
			desc={
				<>
					{t("settings.deleteSettingDescPrefix")}{" "}
					<span className="font-semibold">{setting?.key}</span>{" "}
					{t("settings.deleteSettingDescSuffix")}
				</>
			}
			confirmText={t("common.delete")}
			destructive
			isLoading={deleteSetting.isPending}
			handleConfirm={handleConfirm}
		/>
	);
}
