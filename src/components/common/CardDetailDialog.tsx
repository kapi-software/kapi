// 通用卡片详情弹窗：用于文字过长 / 列表截断时查看完整内容
// Generic card-detail dialog: shows full content when text overflows / is truncated
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ScrollText, X } from "lucide-react";

// 详情弹窗中的分组标题（用于多区块内容展示）
// Section header inside the detail dialog (for multi-section content)
export function DetailSection({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      <h4 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        <ScrollText className="size-3" />
        {title}
      </h4>
      <div className="space-y-1.5">{children}</div>
    </div>
  );
}

// 详情弹窗中的字段行：标签 + 值（值支持长文本，自动换行 / 单行截断 tooltip）
// Field row inside the detail dialog: label + value (auto-wrap / tooltip-truncate)
export function DetailField({
  label,
  value,
  monospace = false,
}: {
  label: string;
  value: React.ReactNode;
  monospace?: boolean;
}) {
  const display =
    value === null || value === undefined || value === "" ? (
      <span className="italic text-muted-foreground/60">—</span>
    ) : (
      value
    );
  return (
    <div className="grid grid-cols-[7rem_1fr] items-start gap-3 rounded-md px-1 py-1.5 text-sm hover:bg-muted/40">
      <span className="pt-0.5 text-xs font-medium text-muted-foreground">
        {label}
      </span>
      <span
        className={
          monospace
            ? "break-all font-mono text-xs leading-relaxed"
            : "break-all leading-relaxed"
        }
      >
        {display}
      </span>
    </div>
  );
}

export function CardDetailDialog({
  open,
  onOpenChange,
  title,
  description,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] gap-0 overflow-hidden p-0 sm:max-w-2xl">
        {/* 头部：标题 + 描述 + 关闭按钮 / Header: title + description + close */}
        <DialogHeader className="border-b bg-muted/30 px-6 py-4">
          <DialogTitle className="text-base">{title}</DialogTitle>
          {description && (
            <DialogDescription className="font-mono text-xs">
              {description}
            </DialogDescription>
          )}
        </DialogHeader>

        {/* 内容区：滚动 / Scrollable body */}
        <div className="max-h-[calc(85vh-9rem)] space-y-4 overflow-y-auto px-6 py-4">
          {children}
        </div>

        {/* 底部：分隔 + 关闭按钮 / Footer: separator + close */}
        <div className="flex items-center justify-between border-t bg-background px-6 py-3 text-xs text-muted-foreground">
          <span>Esc 关闭</span>
          <DialogClose asChild>
            <Button variant="outline" size="sm">
              <X className="size-3.5" />
              {t("common.cancel")}
            </Button>
          </DialogClose>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// 默认导出分隔线供外部组合 / Default export separator for convenience
export { Separator };