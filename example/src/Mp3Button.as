class Mp3Button extends MovieClip {
    function onPress() {
        var sound = new Sound();
        sound.attachSound("test_audio.mp3");
        sound.start();
    }
    function onEnterFrame() {
        trace(Math.random());
    }
}