---
Title: "Operator responsibilities"
Description: "Responsibilities for people who operate a server used with Ichoi."
---
<article class="prose">
<p class="eyebrow">For operators</p>
<h1>Your server sets the local rules.</h1>
<p>Ichoi clients send account and media requests only to the server selected by the user. You operate that server and control its data.</p>
<h2>Tell users clearly</h2><ul><li>Publish your server URL and support contact.</li><li>Explain what data you collect, why you collect it, and how long you keep it.</li><li>Explain how users can delete accounts and ask questions about their data.</li><li>Tell users which media is available and why they can use it.</li></ul>
<h2>Protect the service</h2><ul><li>Use HTTPS and protect database and session secrets.</li><li>Keep server software and dependencies updated.</li><li>Use authenticated identity for playlist ownership. Do not accept an owner supplied by a client.</li><li>Review reports for playlists and accounts. Apply your content rules consistently.</li></ul>
<p>The Ichoi developer cannot delete accounts, inspect reports, or change data on your server.</p>
</article>
