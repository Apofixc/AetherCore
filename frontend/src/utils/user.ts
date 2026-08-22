/**
 * Вычисляет двухбуквенные инициалы пользователя для отображения в аватаре.
 * Если задано полное имя (ФИО) — берет первые буквы слов (например, 'Иван Иванов' -> 'ИИ', 'System Administrator' -> 'SA').
 * Если задан только логин — берет первые 2 символа логина.
 * В случае отсутствия данных возвращает 'OP'.
 */
export function getUserInitials(fullName?: string | null, username?: string | null): string {
  if (fullName && fullName.trim()) {
    const parts = fullName.trim().split(/\s+/).filter(Boolean)
    if (parts.length >= 2) {
      return (parts[0][0] + parts[1][0]).toUpperCase()
    }
    if (parts.length === 1 && parts[0].length > 0) {
      return parts[0].slice(0, 2).toUpperCase()
    }
  }

  if (username && username.trim()) {
    return username.trim().slice(0, 2).toUpperCase()
  }

  return 'OP'
}
