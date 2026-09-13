---
Title: "Server setup"
Description: "Prepare a compatible Ichoi server and media library."
---
<article class="prose">
<p class="eyebrow">For server operators</p>
<h1>Run the service that your users select.</h1>
<p class="lead">The server is the source of truth. Use HTTPS, protect its secrets, and provide only media that you can lawfully provide.</p>
<h2>Before users connect</h2>
<ol><li>Deploy the compatible server on infrastructure that you control.</li><li>Set a public HTTPS URL. Use a valid certificate and keep the server and dependencies updated.</li><li>Configure the database, media path, session lifetime, and administrator account.</li><li>Add media with a licence that permits your planned use. Ichoi supplies no music.</li><li>Test sign-in, browsing, playback, playlist ownership, reporting, and account deletion.</li></ol>
<h2>Operator responsibilities</h2>
<p>You set the privacy notice, support contact, log retention, account policy, media policy, and report review process for your server. You must answer user requests about data on your server. Read the <a href="operator.html">operator responsibilities</a> page.</p>
<p>Do not trust an owner value from a client. The server derives playlist ownership from the authenticated account. Reports target a playlist or an account, and the server operator reviews them.</p>
</article>
