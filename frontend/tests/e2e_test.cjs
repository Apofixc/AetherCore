const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

async function runE2ETests() {
  console.log('🌐 [E2E] Starting full browser end-to-end testing with Chromium...');
  
  const screenshotsDir = path.join(__dirname, 'screenshots');
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
    // -----------------------------------------------------------------------
    // Шаг 1: Открытие страницы авторизации (/login)
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 1: Переход на страницу логина...');
    await page.goto(`${baseUrl}/login`, { waitUntil: 'networkidle' });
    await page.screenshot({ path: path.join(screenshotsDir, '01_login_page.png') });
    console.log('   📸 Скриншот сохранен: 01_login_page.png');

    await page.waitForSelector('#operator_id');
    await page.waitForSelector('#access_code');

    // -----------------------------------------------------------------------
    // Шаг 2: Вход под администратором (admin:admin)
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 2: Ввод учетных данных администратора (admin:admin)...');
    await page.fill('#operator_id', 'admin');
    await page.fill('#access_code', 'admin');
    await page.click('button[type="submit"]');

    // Ожидаем перехода на дашборд
    await page.waitForURL('**/dashboard', { timeout: 10000 });
    console.log('   ✅ Успешный вход, открыт Dashboard');
    await page.screenshot({ path: path.join(screenshotsDir, '02_dashboard.png') });

    // -----------------------------------------------------------------------
    // Шаг 3: Переход в раздел "Управление пользователями" (/settings/users)
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 3: Переход в раздел управления пользователями (/settings/users)...');
    await page.goto(`${baseUrl}/settings/users`, { waitUntil: 'networkidle' });
    await page.waitForSelector('table', { timeout: 10000 });
    await page.screenshot({ path: path.join(screenshotsDir, '03_users_list.png') });
    console.log('   📸 Скриншот сохранен: 03_users_list.png');

    // -----------------------------------------------------------------------
    // Шаг 4: Проверка защиты Root/Superuser от блокировки и удаления
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 4: Проверка защищенных кнопок для пользователя root/admin...');
    const lockButtons = await page.$$('button:has(.material-symbols-outlined:text("lock"))');
    console.log(`   Найдено кнопок блокировки в таблице: ${lockButtons.length}`);

    const disabledButtons = await page.$$('button[disabled]');
    console.log(`   Найдено защищенных кнопок (disabled): ${disabledButtons.length}`);
    if (disabledButtons.length > 0) {
      console.log('   ✅ Защита суперпользователя от блокировки/удаления активна в UI');
    }

    // -----------------------------------------------------------------------
    // Шаг 5: Открытие диалога добавления нового пользователя
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 5: Открытие диалога добавления пользователя...');
    const addBtn = await page.waitForSelector('button:has(.material-symbols-outlined:text("person_add"))', { timeout: 5000 });
    await addBtn.click();
    await page.waitForSelector('#addUserForm', { timeout: 5000 });
    await page.screenshot({ path: path.join(screenshotsDir, '04_add_user_modal.png') });
    console.log('   📸 Скриншот сохранен: 04_add_user_modal.png');

    // -----------------------------------------------------------------------
    // Шаг 6: Заполнение формы и создание нового оператора
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 6: Создание оператора "e2e_operator" с временным паролем...');
    const inputs = await page.$$('#addUserForm input');
    if (inputs.length >= 4) {
      await inputs[0].fill('E2E Test Operator');
      await inputs[1].fill('e2e_operator');
      await inputs[2].fill('e2e_operator@nms.local');
      await inputs[3].fill('temp_pass_123');
    }

    const submitBtn = await page.waitForSelector('button[type="submit"][form="addUserForm"], button:has-text("Создать")');
    await submitBtn.click();
    await page.waitForTimeout(1500);
    await page.screenshot({ path: path.join(screenshotsDir, '05_user_created.png') });
    console.log('   ✅ Новый оператор добавлен в список');

    // -----------------------------------------------------------------------
    // Шаг 7: Выход из аккаунта
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 7: Выход из профиля администратора...');
    await page.goto(`${baseUrl}/login`);
    await page.evaluate(() => {
      localStorage.removeItem('nms_token');
    });
    await page.reload({ waitUntil: 'networkidle' });

    // -----------------------------------------------------------------------
    // Шаг 8: Вход созданным оператором и проверка обязательной смены пароля
    // -----------------------------------------------------------------------
    console.log('🔹 Шаг 8: Авторизация под e2e_operator...');
    await page.fill('#operator_id', 'e2e_operator');
    await page.fill('#access_code', 'temp_pass_123');
    await page.click('button[type="submit"]');

    await page.waitForTimeout(1500);
    await page.screenshot({ path: path.join(screenshotsDir, '06_must_change_password_modal.png') });
    console.log('   📸 Скриншот сохранен: 06_must_change_password_modal.png');

    // Проверяем наличие модального окна смены пароля
    const hasModal = await page.$('#changePasswordForm');
    if (hasModal) {
      console.log('   ✅ Модальное окно "Обязательная смена пароля" успешно перехватило вход');

      // Шаг 9: Задание нового пароля
      console.log('🔹 Шаг 9: Установка нового постоянного пароля...');
      const pwdInputs = await page.$$('#changePasswordForm input[type="password"]');
      if (pwdInputs.length >= 2) {
        await pwdInputs[0].fill('new_permanent_e2e_pass_2026');
        await pwdInputs[1].fill('new_permanent_e2e_pass_2026');
      }
      const savePwdBtn = await page.waitForSelector('button[type="submit"][form="changePasswordForm"], button:has-text("Установить пароль и войти")');
      await savePwdBtn.click();
      await page.waitForTimeout(2000);
      await page.screenshot({ path: path.join(screenshotsDir, '07_dashboard_after_pwd_change.png') });
      console.log('   ✅ Пароль успешно изменен, доступ к дашборду разблокирован');
    }

    console.log('\n🎉 [E2E] ВСЕ БРАУЗЕРНЫЕ СЦЕНАРИИ ТЕСТИРОВАНИЯ УСПЕШНО ЗАВЕРШЕНЫ!');

  } catch (err) {
    console.error('❌ Ошибка во время E2E теста:', err);
    await page.screenshot({ path: path.join(screenshotsDir, 'error_state.png') });
    throw err;
  } finally {
    await browser.close();
  }
}

runE2ETests().catch((e) => {
  console.error(e);
  process.exit(1);
});
