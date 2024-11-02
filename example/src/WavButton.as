class WavButton extends MovieClip {
    function onPress() {
        var sound = new Sound();
        sound.attachSound("pickupCoin.wav");
        sound.start();
    }
}