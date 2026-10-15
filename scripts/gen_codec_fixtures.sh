#!/usr/bin/env sh
# Regenerates testdata/codecs/: one tiny clip per container / codec that
# mkff-codecs decodes in pure Rust. ffmpeg is only the *encoder* for these
# fixtures (dev-time tool); nothing in MKFF links or calls it.
#
#   scripts/gen_codec_fixtures.sh            # writes into testdata/codecs
set -eu
out="$(dirname "$0")/../testdata/codecs"
mkdir -p "$out"
ff="ffmpeg -hide_banner -loglevel error -y"
v="-f lavfi -i testsrc2=size=64x48:rate=24:duration=0.25"
a="-f lavfi -i sine=frequency=440:sample_rate=48000:duration=0.25"
v256="-f lavfi -i testsrc2=size=256x128:rate=24:duration=0.125"
a44="-f lavfi -i sine=frequency=440:sample_rate=44100:duration=0.25"

# Video elementary streams
$ff $v -c:v libvpx -b:v 200k "$out/vp8.ivf"
$ff $v -c:v libvpx-vp9 -b:v 200k -row-mt 0 "$out/vp9.ivf"
$ff $v -c:v libaom-av1 -cpu-used 8 -b:v 200k "$out/av1.ivf"
$ff $v -c:v libx264 -profile:v high -bf 2 -pix_fmt yuv420p -bsf:v h264_mp4toannexb -f h264 "$out/h264_high.h264"

# Containers (video + audio)
$ff $v $a -c:v libvpx-vp9 -b:v 200k -c:a libopus -b:a 48k "$out/vp9_opus.webm"
$ff $v $a -c:v libvpx -b:v 200k -c:a libvorbis "$out/vp8_vorbis.webm"
$ff $v $a -c:v libaom-av1 -cpu-used 8 -b:v 200k -c:a aac -b:a 64k "$out/av1_aac.mp4"
$ff $v $a -c:v libx265 -x265-params log-level=none -pix_fmt yuv420p10le -c:a ac3 -b:a 96k "$out/hevc10_ac3.mkv"
$ff $v $a -c:v mpeg2video -b:v 400k -c:a mp2 -b:a 128k -f mpegts "$out/mpeg2_mp2.ts"
$ff $v $a -c:v libx264 -bf 2 -pix_fmt yuv420p -c:a aac -b:a 64k -f mpegts "$out/h264_aac.ts"
$ff $v $a -c:v prores_ks -profile:v 3 -pix_fmt yuv422p10le -c:a pcm_s16le "$out/prores_hq.mov"
$ff $v -c:v prores_ks -profile:v 4 -pix_fmt yuva444p10le -alpha_bits 16 "$out/prores_4444_alpha.mov"
# DNxHR needs at least 256x120
$ff $v256 $a -c:v dnxhd -profile:v dnxhr_lb -pix_fmt yuv422p -c:a pcm_s16le -shortest "$out/dnxhr.mxf"
$ff $v $a -c:v mjpeg -q:v 5 -pix_fmt yuvj420p -c:a pcm_s16le "$out/mjpeg_pcm.avi"

# Audio only
$ff $a44 -c:a flac "$out/tone.flac"
$ff $a44 -c:a libmp3lame -b:a 64k "$out/tone.mp3"
$ff $a -c:a libopus -b:a 48k "$out/tone.opus"
$ff $a -c:a aac -b:a 64k -f adts "$out/tone.aac"
$ff $a -c:a eac3 -b:a 96k "$out/tone_eac3.mka"
$ff $a44 -c:a alac "$out/tone_alac.m4a"

ls -l "$out"
