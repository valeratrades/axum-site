// assets/script.js

document.querySelectorAll('.resizer').forEach(resizer => {
    const resizable = resizer.parentElement;
    let startX, startWidth;

    resizer.addEventListener('mousedown', initDrag, false);

    function initDrag(e) {
        startX = e.clientX;
        startWidth = parseInt(document.defaultView.getComputedStyle(resizable).width, 10);
        document.documentElement.addEventListener('mousemove', doDrag, false);
        document.documentElement.addEventListener('mouseup', stopDrag, false);
    }

    function doDrag(e) {
        resizable.style.width = (startWidth + e.clientX - startX) + 'px';
    }

    function stopDrag() {
        document.documentElement.removeEventListener('mousemove', doDrag, false);
        document.documentElement.removeEventListener('mouseup', stopDrag, false);
    }
});
