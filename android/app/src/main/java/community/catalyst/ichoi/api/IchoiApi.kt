package community.catalyst.ichoi.api

import community.catalyst.csilgen.generated.*
import community.catalyst.ichoi.storage.ServerProfile
import community.catalyst.ichoi.ui.ReportValidation

/** Handwritten facade around generated clients. Only allowed browsing, playback, and account calls are exposed. */
class IchoiApi(profile: ServerProfile, token: String?) {
    private val session = SessionAsyncClient(HttpCsilTransport(profile, token))
    private val library = LibraryAsyncClient(HttpCsilTransport(profile, token))

    suspend fun authenticate(request: AuthRequest): SessionInfo = session.authenticate(request)
    suspend fun whoAmI(): SessionInfo = session.whoami(Page())
    suspend fun signOut(): Ok = session.logout(Page())
    suspend fun deleteAccount(handle: String): Ok = session.deleteAccount(DeleteAccountRequest(handle))
    suspend fun libraries(): LibrariesResponse = library.listLibraries(Page())
    suspend fun albums(libraryKind: Library, offset: ULong = 0uL): AlbumsResponse =
        library.listAlbums(BrowseRequest(libraryKind, offset, 100uL))
    suspend fun album(id: String): AlbumDetail = library.getAlbum(AlbumRequest(id))
    suspend fun playlists(): PlaylistsResponse = library.listPlaylists(BrowseRequest())
    suspend fun playlist(id: String): PlaylistDetail = library.getPlaylist(PlaylistRequest(id))
    suspend fun coverArt(id: String): CoverArt = library.getCoverArt(CoverArtRequest(id))
    suspend fun report(targetType: ContentReportTargetType, targetId: String, reason: ContentReportReason, details: String?): ContentReport {
        val clean = ReportValidation.validate(targetId, details)
        return library.reportContent(ReportContentRequest(targetType, targetId, reason, clean))
    }
}
