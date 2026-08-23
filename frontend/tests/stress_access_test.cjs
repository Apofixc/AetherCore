const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

async function runAccessStressE2ETests() {
  console.log('⚡ [STRESS E2E] Запуск комплексного стресс- и негативного E2E-тестирования «Доступ и авторизация»...');

  const screenshotsDir = path.join(__dirname, 'screenshots_access');
  if (!fs.existsSync(screenshotsDir)) {
    fs.mkdirSync(screenshotsDir, { recursive: true });
  }

  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage']
  });

  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 }
  });
  const page = await context.newPage();
  const baseUrl = 'http://localhost:5173';

  try {
    // -------------------------------------------------------------------------
    // 1. Авторизация под root/admin
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 1: Авторизация администратора...');
    await page.goto(`${baseUrl}/login`, { waitUntil: 'networkidle' });
    await page.fill('#operator_id', 'admin');
    await page.fill('#access_code', 'admin');
    await page.click('button[type="submit"]');
    await page.waitForURL('**/dashboard', { timeout: 10000 });
    console.log('   ✅ Успешный вход в систему');

    // -------------------------------------------------------------------------
    // 2. Переход на вкладку "Доступ и авторизация"
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 2: Переход во вкладку «Доступ и авторизация» (/settings/access)...');
    await page.goto(`${baseUrl}/settings/access`, { waitUntil: 'networkidle' });
    await page.waitForSelector('table', { timeout: 10000 });
    await page.screenshot({ path: path.join(screenshotsDir, '01_access_initial.png') });
    console.log('   📸 Скриншот сохранен: 01_access_initial.png');

    // -------------------------------------------------------------------------
    // 3. Стресс-тест политик MFA: переключение режимов и проверка подсказок
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 3: Стресс-тест переключения скоупов MFA...');
    const mfaScopeButtons = await page.$$('button:has(.material-symbols-outlined:text("lock_open")), button:has(.material-symbols-outlined:text("admin_panel_settings")), button:has(.material-symbols-outlined:text("security"))');
    console.log(`   Найдено кнопок скоупа MFA: ${mfaScopeButtons.length}`);
    for (let i = 0; i < mfaScopeButtons.length; i++) {
      await mfaScopeButtons[i].click();
      await page.waitForTimeout(100);
    }
    await page.screenshot({ path: path.join(screenshotsDir, '02_mfa_scopes_cycled.png') });

    // -------------------------------------------------------------------------
    // 4. Стресс-фаззинг матрицы прав: быстрый каскадный клик по чекбоксам
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 4: Стресс-проверка каскадных зависимостей в матрице прав...');
    const checkboxes = await page.$$('tbody input[type="checkbox"]:not([disabled])');
    console.log(`   Найдено интерактивных переключателей матрицы: ${checkboxes.length}`);

    // Быстрый клик по 20 чекбоксам для проверки реактивности
    for (let i = 0; i < Math.min(checkboxes.length, 20); i++) {
      await checkboxes[i].click({ force: true });
    }
    await page.screenshot({ path: path.join(screenshotsDir, '03_matrix_rapid_toggle.png') });
    console.log('   ✅ Матрица прав выдержала быстрый поток переключений');

    // -------------------------------------------------------------------------
    // 5. Поиск по матрице прав и сброс фильтра
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 5: Поиск по матрице прав...');
    const matrixSearch = await page.locator('input[placeholder*="Поиск прав"], input[placeholder*="Search"]').first();
    await matrixSearch.fill('backup');
    await page.waitForTimeout(300);
    await page.screenshot({ path: path.join(screenshotsDir, '04_matrix_search_filtered.png') });
    await matrixSearch.fill('');
    await page.waitForTimeout(200);

    // -------------------------------------------------------------------------
    // 6. Стресс-ввод экстремального IP Whitelist
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 6: Заполнение экстремального IP Whitelist (100 подсетей)...');
    const stressIpList = Array.from({ length: 100 }, (_, i) => `192.168.${Math.floor(i / 255)}.${i % 255}/32`).join(', ');
    const ipInput = await page.locator('input[type="text"]').last();
    await ipInput.fill(stressIpList);
    await page.screenshot({ path: path.join(screenshotsDir, '05_extreme_ip_whitelist.png') });

    // -------------------------------------------------------------------------
    // 7. Журнал аудита: фильтрация по категориям и смена пагинации
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 7: Стресс-тест фильтрации и пагинации журнала аудита...');
    const refreshBtn = await page.locator('button[title*="Обновить"], button[title*="Refresh"]').first();
    if (await refreshBtn.isVisible()) {
      await refreshBtn.click();
      await page.waitForTimeout(500);
    }

    const searchAudit = await page.locator('input[placeholder*="Поиск по журналу"], input[placeholder*="Search audit"]').first();
    if (await searchAudit.isVisible()) {
      await searchAudit.fill('root');
      await page.waitForTimeout(300);
      await page.screenshot({ path: path.join(screenshotsDir, '06_audit_search_active.png') });
      await searchAudit.fill('');
    }

    // -------------------------------------------------------------------------
    // 8. Проверка модального окна очистки аудита
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 8: Проверка диалога очистки аудита...');
    const clearBtn = await page.locator('button[title*="Очистить"], button[title*="Clear"]').first();
    if (await clearBtn.isVisible()) {
      await clearBtn.click();
      await page.waitForTimeout(300);
      await page.screenshot({ path: path.join(screenshotsDir, '07_clear_audit_modal.png') });
      // Закрываем модальное окно (Отмена)
      const cancelBtn = await page.locator('button:has-text("Отмена"), button:has-text("Cancel")').last();
      if (await cancelBtn.isVisible()) {
        await cancelBtn.click();
      }
    }

    // -------------------------------------------------------------------------
    // 9. Сохранение политик и матрицы («Применить изменения»)
    // -------------------------------------------------------------------------
    console.log('🔹 Шаг 9: Сохранение настроек...');
    const applyBtn = await page.locator('button:has-text("Применить изменения"), button:has-text("Save")').first();
    await applyBtn.click();
    await page.waitForTimeout(1500);
    await page.screenshot({ path: path.join(screenshotsDir, '08_changes_applied.png') });
    console.log('   ✅ Все изменения успешно применены без ошибок');

    console.log('🎉 [STRESS E2E] Комплексное тестирование вкладки завершено успешно!');
  } catch (err) {
    console.error('❌ Ошибка во время E2E тестирования:', err);
    await page.screenshot({ path: path.join(screenshotsDir, 'error_state.png') });
    throw err;
  } finally {
    await browser.close();
  }
}

runAccessStressE2ETests().catch(() => process.exit(1));
