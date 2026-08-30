// 通用确认对话框：基于 shadcn Dialog，含标题/描述/确认按钮（可标记 destructive）
// Generic confirm dialog: built on shadcn Dialog with title/description/confirm button
// （可选 destructive 样式）；用 onOpenChange 控制显隐
// Optional destructive styling; controlled via onOpenChange
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export interface ConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  // 按钮文案 / Button label
  confirmLabel?: string
  cancelLabel?: string
  // destructive 强调色（删除等不可逆操作）
  // Destructive emphasis (e.g. for irreversible actions)
  destructive?: boolean
  onConfirm: () => void | Promise<void>
  // 异步确认时禁用按钮防止重复点击
  // Disable the button while the async action runs to prevent double-submits
  busy?: boolean
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  cancelLabel,
  destructive,
  onConfirm,
  busy,
}: ConfirmDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && (
            <DialogDescription>{description}</DialogDescription>
          )}
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={busy}
          >
            {cancelLabel ?? t("common.cancel")}
          </Button>
          <Button
            variant={destructive ? "destructive" : "default"}
            onClick={() => onConfirm()}
            disabled={busy}
          >
            {confirmLabel ?? t("common.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}