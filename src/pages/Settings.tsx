// 设置页：Phase 1 最小实现（语言 + 主题 + Dock 开关）
// Settings page: Phase 1 minimal implementation (language + theme + dock toggle)
// 设置项清单见 docs/PANEL.md
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useSettingsStore } from "@/stores/settings";
import { normalizeLanguage, SUPPORTED_LANGUAGES } from "@/i18n";
import type { ThemeMode } from "@/lib/settings";

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

export default function Settings() {
  const { t } = useTranslation();
  const settings = useSettingsStore((s) => s.settings);
  const updateSetting = useSettingsStore((s) => s.updateSetting);
  const resetSettings = useSettingsStore((s) => s.resetSettings);

  return (
    <div className="mx-auto max-w-2xl space-y-4">
      <div>
        <h1 className="text-2xl font-bold">{t("settings.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("settings.subtitle")}</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.languageTitle")}</CardTitle>
          <CardDescription>{t("settings.languageDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="flex gap-2">
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
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.themeTitle")}</CardTitle>
          <CardDescription>{t("settings.themeDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="flex gap-2">
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
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.dockTitle")}</CardTitle>
          <CardDescription>{t("settings.dockDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="flex items-center justify-between">
          <span className={cn("text-sm", !settings.dock_enabled && "text-muted-foreground")}>
            {settings.dock_enabled ? t("settings.dockOn") : t("settings.dockOff")}
          </span>
          <Switch
            checked={settings.dock_enabled}
            onCheckedChange={(v) => updateSetting("dock_enabled", v)}
          />
        </CardContent>
      </Card>

      <div className="flex justify-end">
        <Button variant="outline" size="sm" onClick={() => resetSettings()}>
          {t("settings.reset")}
        </Button>
      </div>
    </div>
  );
}
