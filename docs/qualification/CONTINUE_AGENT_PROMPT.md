# Aura — Universal “ממשיכים” Prompt (Agents A/B/C)
**Copy this complete text to an agent at session start, or send "ממשיכים" after the agent has adopted it.** This instruction is *permanent for the lane* and must be re-applied at every subsequent phase, even if the entire previous conversation was compressed or lost. It never authorizes merge or another agent's code modification.

```text
אני ממשיך את הפיתוח ההנדסי הפעיל של Aura Lang. זו פעימת ביצוע נוספת, לא חזרה לתכנון וללא החלפת תחום הבעלות שלי.

Repository: https://github.com/danielforface/aura-lang
Coordination: https://github.com/danielforface/aura-lang/issues/59
Integration / PR debt: https://github.com/danielforface/aura-lang/issues/109
Current policy documentation (READ from live branch PR #95 even if absent from my own branch):
https://github.com/danielforface/aura-lang/pull/95
https://github.com/danielforface/aura-lang/blob/plan/aura-quantum-full-stack-v3/AGENTS.md
https://github.com/danielforface/aura-lang/blob/plan/aura-quantum-full-stack-v3/docs/qualification/PHASE_LIFECYCLE_V1.md
https://github.com/danielforface/aura-lang/blob/plan/aura-quantum-full-stack-v3/docs/qualification/CROSS_AGENT_COORDINATION_V1.md
https://github.com/danielforface/aura-lang/blob/plan/aura-quantum-full-stack-v3/docs/qualification/DELIVERY_OPERATING_POLICY_V1.md
https://github.com/danielforface/aura-lang/blob/plan/aura-quantum-full-stack-v3/docs/qualification/OPERATING_PROTOCOL.md

**בכל פעימה מחדש**, ולא רק בפתיחת הצ'אט:
1. זהה אוטומטית את תחום הבעלות שלי מתוך ה-PR/Issue הפעיל: A=Core Q03, B=Lumina UI/IBus, C=Quantum Q28, או תחום אחר שאושר מפורשות. קרא את docs/qualification/agents/AGENT_{A/B/C}_..._ORDER.md הרלוונטי וגם את תוכנית V1/V2/V3 וסעיף Q שלי. לעולם אל תשתמש ב-HEAD ישן רק משום שמופיע בפרומפט.
2. קרא תחילה את GitHub החי: PR ראשי, HEAD/Base, CI, קבצי השינוי, Issues, ולפחות שני תחומי האגנטים האחרים כאשר יש אפשרות לממשק משותף. שלוף compare כנגד main וכנגד ענף הבסיס; GitHub mergeable ל-PR-הורה אינו אישור למזג ל-main.
3. בצע לפני כתיבה סיווג D0–D5 לפי מסמך CROSS_AGENT_COORDINATION_V1, לרבות קבצים, API/ABI, IR, תקשורת, חוזי סמנטיקה, מסמכי אינטגרציה, היסטוריית ענפים ותקציב CI. אם יש D2/D3/D4: בקש חוזה/הכרעת אינטגרציה ב-Issue אחד, הימנע מכתיבה של אחר, אך המשך עבודה עצמאית שאינה חסומה.
4. השתמש בהשלמות שכבר קיימות בענף: אל תתחיל מחדש ואל תפתח מחקר נוסף שהוכרע. הגדר יעד אחד: פונקציה שימושית, תיקון באג מוכח, שיפור מדיד או שחרור חסם אינטגרציה. כתוב שינוי קוד ממשי ובדיקות במידת הרלוונטיות. פיתוח מחקרי/פורמלי עצמאי חייב להפיק ראיה או כלי בעל שימוש אמיתי ולהיות מסומן בהתאם.
5. בצע בדיקות ממוקדות מקומיות לפני CI. אל תשלח E0599 מכוון או קוד שלא מתקמפל ל-GitHub Actions רק כדי להציג RED. אל תפתח Draft צאצא חדש לפעימה קטנה. עבוד ב-PR הפעיל עד למסירת יחידה משמעותית. אל תבצע force push, merge, rebase עיוור או סגירת PR ללא אישור.
6. לפני Commit/Push ובסיום, חזור לקרוא וליישם את PHASE_LIFECYCLE_V1 ואת CROSS_AGENT_COORDINATION_V1; אמת שה-HEAD, קונפליקטים וחוזים לא השתנו. בחן תוצאה גם ב-PR מול הבסיס וגם על עץ אינטגרציה מתאים לפני הצהרת "ready for main".
7. סכם בעברית: Agent/Issue/PR/HEAD/Base, BEFORE→AFTER מוצר, קוד ובדיקות, D0–D5 והחלטת חפיפה, CI אמיתי, סטטוס אינטגרציה, הפעימה הבאה בתוך אותו PR. אל תציג ניסוי כסמכות ייצור, Proof כסימולציה או CI ירוק כ-merge.

זהו חוזה המשך מחייב מחדש בכל "ממשיכים". אם הוראה מקומית ישנה מתנגשת בהנחיות החדשות, העדף את מדיניות Production-First והחפיפות המשותפות כפי שפורסמה ב-PR #95. הצג חסימות אמיתיות ולא המצאות, והמשך מיד בביצוע עבודה הנדסית לפי הבעלות שלי.
```

**Applicability:** Existing A/B/C agent chats should be given this text once. Thereafter, a plain "ממשיכים" triggers Stage 0 anew. This is a documented working convention, not a background service and cannot force a disconnected chat to read files unless it follows the instruction.
