import { useEffect, useState } from "react";
import { monthFromDateName, todayDateName, yearFromDateName } from "../utils/streamDates.js";

const MONTH_NAMES = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];
const DAY_NAMES = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

function buildCalendarDays(year, month) {
  const firstDayOfWeek = new Date(year, month, 1).getDay();
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const daysInPrevMonth = new Date(year, month, 0).getDate();
  const days = [];

  for (let i = firstDayOfWeek - 1; i >= 0; i -= 1) {
    const dayOfMonth = daysInPrevMonth - i;
    const prevMonth = month === 0 ? 11 : month - 1;
    const prevYear = month === 0 ? year - 1 : year;
    days.push({
      dayOfMonth,
      isCurrentMonth: false,
      dateName: `${String(prevYear).padStart(4, "0")}_${String(prevMonth + 1).padStart(2, "0")}_${String(dayOfMonth).padStart(2, "0")}`,
    });
  }

  for (let dayOfMonth = 1; dayOfMonth <= daysInMonth; dayOfMonth += 1) {
    days.push({
      dayOfMonth,
      isCurrentMonth: true,
      dateName: `${String(year).padStart(4, "0")}_${String(month + 1).padStart(2, "0")}_${String(dayOfMonth).padStart(2, "0")}`,
    });
  }

  const remaining = 42 - days.length;
  for (let dayOfMonth = 1; dayOfMonth <= remaining; dayOfMonth += 1) {
    const nextMonth = month === 11 ? 0 : month + 1;
    const nextYear = month === 11 ? year + 1 : year;
    days.push({
      dayOfMonth,
      isCurrentMonth: false,
      dateName: `${String(nextYear).padStart(4, "0")}_${String(nextMonth + 1).padStart(2, "0")}_${String(dayOfMonth).padStart(2, "0")}`,
    });
  }

  return days;
}

export default function SidebarCalendar({ selectedDate, streamPagesByDate, onSelectDate }) {
  const [viewYear, setViewYear] = useState(() => {
    if (selectedDate && selectedDate.length >= 7) {
      return yearFromDateName(selectedDate);
    }
    return new Date().getFullYear();
  });
  const [viewMonth, setViewMonth] = useState(() => {
    if (selectedDate && selectedDate.length >= 7) {
      return monthFromDateName(selectedDate);
    }
    return new Date().getMonth();
  });

  useEffect(() => {
    if (!selectedDate || selectedDate.length < 7) {
      return;
    }
    setViewYear(yearFromDateName(selectedDate));
    setViewMonth(monthFromDateName(selectedDate));
  }, [selectedDate]);

  const days = buildCalendarDays(viewYear, viewMonth);
  const today = todayDateName();

  function navigateMonth(delta) {
    let nextMonth = viewMonth + delta;
    let nextYear = viewYear;
    if (nextMonth < 0) {
      nextMonth = 11;
      nextYear -= 1;
    }
    if (nextMonth > 11) {
      nextMonth = 0;
      nextYear += 1;
    }
    setViewMonth(nextMonth);
    setViewYear(nextYear);
  }

  return (
    <div className="stream-calendar">
      <div className="stream-calendar-nav">
        <button
          className="stream-calendar-nav-btn"
          type="button"
          aria-label="Previous month"
          onClick={() => navigateMonth(-1)}
        >
          {"<"}
        </button>
        <span className="stream-calendar-month-label">{MONTH_NAMES[viewMonth]} {viewYear}</span>
        <button
          className="stream-calendar-nav-btn"
          type="button"
          aria-label="Next month"
          onClick={() => navigateMonth(1)}
        >
          {">"}
        </button>
      </div>
      <div className="stream-calendar-day-headers">
        {DAY_NAMES.map((name) => (
          <span key={name} className="stream-calendar-day-header">{name}</span>
        ))}
      </div>
      <div className="stream-calendar-grid">
        {days.map((day) => {
          const streamNames = streamPagesByDate.get(day.dateName) ?? [];
          const hasDiary = streamNames.includes("diary");
          const hasJournals = streamNames.includes("journals");
          const hasExtra = streamNames.some((name) => name !== "diary" && name !== "journals");
          const isSelected = day.dateName === selectedDate;
          const isToday = day.dateName === today;
          const hasAny = hasDiary || hasJournals || hasExtra;

          return (
            <button
              key={day.dateName}
              type="button"
              className={[
                "stream-calendar-day",
                !day.isCurrentMonth ? "stream-calendar-day--other" : "",
                isSelected ? "stream-calendar-day--selected" : "",
                isToday && !isSelected ? "stream-calendar-day--today" : "",
              ].filter(Boolean).join(" ")}
              onClick={() => onSelectDate(day.dateName)}
            >
              <span className="stream-calendar-day-num">{day.dayOfMonth}</span>
              {hasAny ? (
                <span className="stream-calendar-markers">
                  {hasDiary ? <span className="stream-calendar-dot stream-calendar-dot--diary" /> : null}
                  {hasJournals ? <span className="stream-calendar-dot stream-calendar-dot--journals" /> : null}
                  {hasExtra ? <span className="stream-calendar-dot stream-calendar-dot--extra" /> : null}
                </span>
              ) : null}
            </button>
          );
        })}
      </div>
    </div>
  );
}
