// den-dj transports ffmpeg's Opus stream into one LiveKit room. Queue policy
// stays in den-server. Its room-scoped token is passed in the environment.
package main

import (
	"fmt"
	"github.com/livekit/protocol/livekit"
	lksdk "github.com/livekit/server-sdk-go/v2"
	"github.com/pion/webrtc/v4"
	"os"
	"os/signal"
	"syscall"
	"time"
)

func run() error {
	disconnected := make(chan struct{}, 1)
	room, err := lksdk.ConnectToRoomWithToken(os.Getenv("DEN_DJ_URL"), os.Getenv("DEN_DJ_TOKEN"), &lksdk.RoomCallback{
		OnDisconnected: func() {
			select {
			case disconnected <- struct{}{}:
			default:
			}
		},
	}, lksdk.WithAutoSubscribe(false))
	if err != nil {
		return fmt.Errorf("connect failed")
	}
	defer room.Disconnect()
	complete := make(chan struct{}, 1)
	track, err := lksdk.NewLocalReaderTrack(os.Stdin, webrtc.MimeTypeOpus,
		lksdk.ReaderTrackWithFrameDuration(20*time.Millisecond),
		lksdk.ReaderTrackWithOnWriteComplete(func() { complete <- struct{}{} }))
	if err != nil {
		return fmt.Errorf("invalid Opus stream")
	}
	defer track.Close()
	if _, err = room.LocalParticipant.PublishTrack(track, &lksdk.TrackPublicationOptions{
		Name: "Music", Source: livekit.TrackSource_MICROPHONE, Stereo: true,
	}); err != nil {
		return fmt.Errorf("publish failed")
	}
	fmt.Println("READY")
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, os.Interrupt, syscall.SIGTERM)
	defer signal.Stop(stop)
	select {
	case <-complete:
		return nil
	case <-stop:
		return nil
	case <-disconnected:
		return fmt.Errorf("room disconnected")
	}
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
