// 设置页：按 docs/PANEL.md §4.2 分组渲染全部设置项（通用 / 主题 / Dock / 插件）
// Settings page: all settings grouped per docs/PANEL.md §4.2 (general / theme / dock / plugin)
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";
import { useSettingsStore } from "@/stores/settings";
import { normalizeLanguage, SUPPORTED_LANGUAGES } from "@/i18n";
import { ACCENT_PRESETS } from "@/lib/theme";
import type { AppSettings, ThemeMode } from "@/lib/settings";

// 主题选项（值 → i18n key）
// Theme options (value → i18n key)
const THEME_OPTIONS: Array<{ value: ThemeMode; labelKey: string }> = [
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" },
  { value: "system", labelKey: "settings.themeSystem" },
];

// 语言选项展示名（语言包自身的名字，不随界面语言变化）
// Native language display names (do not change with the UI language)
const LANGUAGE_NAMES: Record<string, string> = {
  "zh-CN": "简体中文",
  "en-US": "English",
};

// Dock 动画速度选项
// Dock animation speed options
const SPEED_OPTIONS: Array<{ value: AppSettings["dock_animation_speed"]; labelKey: string }> = [
  { value: "slow", labelKey: "settings.speedSlow" },
  { value: "medium", labelKey: "settings.speedMedium" },
  { value: "fast", labelKey: "settings.speedFast" },
];

// 插件日志级别选项（由低到高）
// Plugin log level options (low to high)
const LOG_LEVEL_OPTIONS: Array<{ value: AppSettings["plugin_log_level"]; labelKey: string }> = [
  { value: "debug", labelKey: "settings.levelDebug" },
  { value: "info", labelKey: "settings.levelInfo" },
  { value: "warn", labelKey: "settings.levelWarn" },
  { value: "error", labelKey: "settings.levelError" },
];

// Dock 位置选项（left 为预留值，见 PANEL.md §4.2）
// Dock position options (left is reserved, see PANEL.md §4.2)
const POSITION_OPTIONS: Array<{ value: AppSettings["dock_position"]; labelKey: string }> = [
  { value: "right", labelKey: "settings.posRight" },
  { value: "left", labelKey: "settings.posLeft" },
];

// 单行设置项：左侧名称 + 说明，右侧控件
// One settings row: name + description on the left, control on the right
function SettingRow({
  label,
  desc,
  dimmed = false,
  children,
}: {
  label: string;
  desc?: string;
  dimmed?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className={cn("flex items-center justify-between gap-6 py-3", dimmed && "opacity-50")}>
      <div className="min-w-0">
        <p className="text-sm font-medium">{label}</p>
        {desc && <p className="mt-0.5 text-xs text-muted-foreground">{desc}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

// 滑杆设置项：名称 + 当前值占一行，滑杆整行铺开
// Slider row: label + live value on one line, slider spanning the full width
function SliderRow({
  label,
  desc,
  valueText,
  dimmed = false,
  children,
}: {
  label: string;
  desc?: string;
  valueText: string;
  dimmed?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className={cn("py-3", dimmed && "opacity-50")}>
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-sm font-medium">{label}</p>
          {desc && <p className="mt-0.5 text-xs text-muted-foreground">{desc}</p>}
        </div>
        {/* 当前值右对齐固定宽度，滑动时不跳动 */}
        {/* Fixed-width right-aligned value so it doesn't jitter while sliding */}
        <span className="w-20 shrink-0 text-right font-mono text-xs text-muted-foreground">
          {valueText}
        </span>
      </div>
      <div className="mt-2">{children}</div>
    </div>
  );
}

// 强调色选择：预设色板 + 自定义取色器
// Accent picker: preset swatches + a custom color input
function AccentPicker({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-wrap items-center gap-2">
      {ACCENT_PRESETS.map((color) => {
        const active = value.toLowerCase() === color.toLowerCase();
        return (
          <button
            key={color}
            type="button"
            title={color}
            onClick={() => onChange(color)}
            className={cn(
              "flex size-7 items-center justify-center rounded-full border transition-transform hover:scale-110",
              active ? "border-foreground/60" : "border-transparent"
            )}
          >
            <span
              className="flex size-5 items-center justify-center rounded-full"
              style={{ backgroundColor: color }}
            >
              {active && <Check className="size-3 text-white mix-blend-difference" />}
            </span>
          </button>
        );
      })}
      {/* 自定义色：原生取色器，input[type=color] 仅接受 #rrggbb */}
      {/* Custom color: native picker; input[type=color] only accepts #rrggbb */}
      <label
        className="flex cursor-pointer items-center gap-1.5 rounded-md border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent/50"
        title={t("settings.accentCustom")}
      >
        <span
          className="size-3.5 rounded-full border border-border"
          style={{ backgroundColor: value }}
        />
        {t("settings.accentCustom")}
        <input
          type="color"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="sr-only"
        />
      </label>
    </div>
  );
}

export default function Settings() {
  const { t } = useTranslation();
  const settings = useSettingsStore((s) => s.settings);
  const updateSetting = useSettingsStore((s) => s.updateSetting);
  const resetSettings = useSettingsStore((s) => s.resetSettings);

  // Dock 未启用时下方细选项整体置灰
  // Fine-grained dock rows are dimmed while the dock is off
  const dockDisabled = !settings.dock_enabled;

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto">
      {/* 通用 / General */}
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.generalTitle")}</CardTitle>
          <CardDescription>{t("settings.generalDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="divide-y">
          <SettingRow label={t("settings.languageTitle")} desc={t("settings.languageDesc")}>
            <div className="flex gap-2">
              {SUPPORTED_LANGUAGES.map((lang) => (
                <Button
                  key={lang}
                  variant={normalizeLanguage(settings.language) === lang ? "default" : "outline"}
                  size="sm"
                  onClick={() => updateSetting("language", lang)}
                >
                  {LANGUAGE_NAMES[lang]}
                </Button>
              ))}
            </div>
          </SettingRow>
          <SettingRow label={t("settings.autoStart")} desc={t("settings.autoStartDesc")}>
            <Switch
              checked={settings.auto_start}
              onCheckedChange={(v) => updateSetting("auto_start", v)}
            />
          </SettingRow>
          <SettingRow label={t("settings.checkUpdate")} desc={t("settings.checkUpdateDesc")}>
            <Switch
              checked={settings.check_update}
              onCheckedChange={(v) => updateSetting("check_update", v)}
            />
          </SettingRow>
        </CardContent>
      </Card>

      {/* 主题 / Theme */}
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.themeTitle")}</CardTitle>
          <CardDescription>{t("settings.themeDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="divide-y">
          <SettingRow label={t("settings.themeMode")} desc={t("settings.themeDesc")}>
            <div className="flex gap-2">
              {THEME_OPTIONS.map(({ value, labelKey }) => (
                <Button
                  key={value}
                  variant={settings.theme === value ? "default" : "outline"}
                  size="sm"
                  onClick={() => updateSetting("theme", value)}
                >
                  {t(labelKey)}
                </Button>
              ))}
            </div>
          </SettingRow>
          <SettingRow label={t("settings.accentColor")} desc={t("settings.accentColorDesc")}>
            <AccentPicker
              value={settings.accent_color}
              onChange={(v) => updateSetting("accent_color", v)}
            />
          </SettingRow>
        </CardContent>
      </Card>

      {/* Dock */}
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.dockTitle")}</CardTitle>
          <CardDescription>{t("settings.dockDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="divide-y">
          <SettingRow
            label={settings.dock_enabled ? t("settings.dockOn") : t("settings.dockOff")}
          >
            <Switch
              checked={settings.dock_enabled}
              onCheckedChange={(v) => updateSetting("dock_enabled", v)}
            />
          </SettingRow>
          <div className={cn(dockDisabled && "pointer-events-none")}>
            <SettingRow label={t("settings.dockPosition")}>
              <Select
                value={settings.dock_position}
                onValueChange={(v) =>
                  updateSetting("dock_position", v as AppSettings["dock_position"])
                }
              >
                <SelectTrigger size="sm" className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {POSITION_OPTIONS.map(({ value, labelKey }) => (
                    <SelectItem key={value} value={value}>
                      {t(labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </SettingRow>
          </div>
          <SliderRow
            label={t("settings.dockHotzoneWidth")}
            desc={t("settings.dockHotzoneDesc")}
            valueText={`${settings.dock_hotzone_width} px`}
            dimmed={dockDisabled}
          >
            <Slider
              value={[settings.dock_hotzone_width]}
              min={6}
              max={24}
              step={1}
              disabled={dockDisabled}
              onValueChange={([v]) => updateSetting("dock_hotzone_width", v)}
            />
          </SliderRow>
          <div className={cn(dockDisabled && "pointer-events-none")}>
            <SettingRow label={t("settings.dockAnimationSpeed")}>
              <Select
                value={settings.dock_animation_speed}
                onValueChange={(v) =>
                  updateSetting("dock_animation_speed", v as AppSettings["dock_animation_speed"])
                }
              >
                <SelectTrigger size="sm" className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {SPEED_OPTIONS.map(({ value, labelKey }) => (
                    <SelectItem key={value} value={value}>
                      {t(labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </SettingRow>
          </div>
          <SliderRow
            label={t("settings.dockExpandDelay")}
            desc={t("settings.dockDelayDesc")}
            valueText={`${settings.dock_expand_delay} ms`}
            dimmed={dockDisabled}
          >
            <Slider
              value={[settings.dock_expand_delay]}
              min={0}
              max={1000}
              step={50}
              disabled={dockDisabled}
              onValueChange={([v]) => updateSetting("dock_expand_delay", v)}
            />
          </SliderRow>
          <SliderRow
            label={t("settings.dockVisibleItems")}
            desc={t("settings.dockVisibleDesc")}
            valueText={String(settings.dock_visible_items)}
            dimmed={dockDisabled}
          >
            {/* 步进 2 保持奇数：弧形布局需要唯一居中位（docs/DOCK.md） */}
            {/* Step 2 keeps counts odd: the arc layout needs a single center slot */}
            <Slider
              value={[settings.dock_visible_items]}
              min={5}
              max={13}
              step={2}
              disabled={dockDisabled}
              onValueChange={([v]) => updateSetting("dock_visible_items", v)}
            />
          </SliderRow>
        </CardContent>
      </Card>

      {/* 插件 / Plugins */}
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.pluginTitle")}</CardTitle>
          <CardDescription>{t("settings.pluginDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="divide-y">
          <SettingRow label={t("settings.pluginAutoUpdate")} desc={t("settings.pluginAutoUpdateDesc")}>
            <Switch
              checked={settings.plugin_auto_update}
              onCheckedChange={(v) => updateSetting("plugin_auto_update", v)}
            />
          </SettingRow>
          <SettingRow
            label={t("settings.pluginSandboxStrict")}
            desc={t("settings.pluginSandboxDesc")}
          >
            <Switch
              checked={settings.plugin_sandbox_strict}
              onCheckedChange={(v) => updateSetting("plugin_sandbox_strict", v)}
            />
          </SettingRow>
          <SettingRow label={t("settings.pluginLogLevel")} desc={t("settings.pluginLogLevelDesc")}>
            <Select
              value={settings.plugin_log_level}
              onValueChange={(v) =>
                updateSetting("plugin_log_level", v as AppSettings["plugin_log_level"])
              }
            >
              <SelectTrigger size="sm" className="w-28">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {LOG_LEVEL_OPTIONS.map(({ value, labelKey }) => (
                  <SelectItem key={value} value={value}>
                    {t(labelKey)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </SettingRow>
        </CardContent>
      </Card>

      <div className="flex items-center justify-between">
        <Separator className="flex-1" />
        <Button variant="outline" size="sm" className="mx-4" onClick={() => resetSettings()}>
          {t("settings.reset")}
        </Button>
        <Separator className="flex-1" />
      </div>
    </div>
  );
}
