export function formatDateTime(iso: string): { date: string; time: string } {
  if (!iso) return { date: "", time: "" };
  const d = new Date(iso);
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const startOfDay = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const diffDays = Math.round((startOfToday.getTime() - startOfDay.getTime()) / 86400000);

  let date: string;
  if (diffDays === 0) date = "Today";
  else if (diffDays === 1) date = "Yesterday";
  else if (diffDays <= 6) date = `${diffDays}d ago`;
  else date = d.toLocaleDateString("en-US", { month: "short", day: "numeric" });

  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return { date, time };
}
