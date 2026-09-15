import { chromium } from 'playwright-core'
const browser = await chromium.launch({executablePath:'/usr/bin/chromium',args:['--no-sandbox','--use-fake-ui-for-media-stream','--use-fake-device-for-media-stream']})
try {
 const context = await browser.newContext({viewport:{width:1440,height:1000},permissions:['camera','microphone'],colorScheme:'dark'})
 const page = await context.newPage()
 await page.goto(process.env.DEN_SMOKE_URL || 'http://localhost:5178')
 await page.getByLabel('Username',{exact:true}).fill('nicholas')
 await page.getByLabel('Password',{exact:true}).fill(process.env.DEN_SMOKE_PASSWORD)
 await page.getByRole('button',{name:'Come in',exact:true}).click()
 await page.locator('a[title="Settings"]').click()
 await page.getByRole('link',{name:'Voice',exact:true}).click()
 await page.getByRole('meter').waitFor()
 await page.waitForTimeout(1000)
 await page.screenshot({path:'docs/shots/pr/feat/av-settings/before.png'})
 console.log('Before captured', await page.locator('h2').allTextContents())
} finally {await browser.close()}
